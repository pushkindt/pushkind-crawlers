use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use pushkind_dantes::domain::product::NewProduct;
use scraper::{Html, Selector};
use tokio::sync::Semaphore;
use url::Url;

use crate::crawlers::build_new_product;
use crate::crawlers::{CrawlerError, CrawlerResult, WebstoreCrawler, build_reqwest_client};

/// Crawler for `101tea.ru` which limits concurrent HTTP requests
/// using a [`Semaphore`].
pub struct WebstoreCrawler101Tea {
    crawler_id: i32,
    base_url: Url,
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
}

impl WebstoreCrawler101Tea {
    /// Creates a new crawler with the given concurrency limit.
    ///
    /// `concurrency` controls how many HTTP requests may be in flight at the
    /// same time. The `crawler_id` is attached to each produced product.
    pub fn new(concurrency: usize, crawler_id: i32) -> CrawlerResult<Self> {
        Ok(Self {
            crawler_id,
            base_url: Url::parse("https://101tea.ru/")
                .map_err(|e| CrawlerError::Build(e.to_string()))?,
            client: build_reqwest_client()?,
            semaphore: Arc::new(Semaphore::new(concurrency)),
        })
    }

    /// Fetches a URL and parses it into [`Html`].
    ///
    /// A permit from the internal [`Semaphore`] is acquired before issuing
    /// the request, enforcing the configured concurrency limit.
    async fn fetch_html(&self, url: &str) -> Option<Html> {
        let _permit = self.semaphore.acquire().await.ok()?;
        let res = self.client.get(url).send().await.ok()?;
        if !res.status().is_success() {
            log::error!("Failed to get URL {}: {}", url, res.status());
            return None;
        }
        let text = res.text().await.ok()?;
        Some(Html::parse_document(&text))
    }

    /// Retrieves all category links from the store's landing page.
    async fn get_category_links(&self) -> Vec<String> {
        let document = match self.fetch_html(self.base_url.as_str()).await {
            Some(doc) => doc,
            None => {
                log::error!("Failed to parse HTML {}", self.base_url);
                return vec![];
            }
        };

        let selector = Selector::parse("a.catalog-nav__link").unwrap();

        document
            .select(&selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                Some(self.base_url.join(href).ok()?.to_string())
            })
            .collect()
    }

    /// For a given category URL, discovers all pagination links, returning
    /// the original URL and any additional pages.
    async fn get_page_links(&self, url: &str) -> Vec<String> {
        let mut result = vec![url.to_string()];
        let document = match self.fetch_html(url).await {
            Some(doc) => doc,
            None => {
                log::error!("Failed to parse HTML {url}");
                return vec![];
            }
        };

        let selector = Selector::parse("div.pagination").unwrap();
        let pagination = match document.select(&selector).next() {
            Some(p) => p,
            None => return result,
        };

        let selector = Selector::parse("a.pagination-links").unwrap();
        let page_links = pagination.select(&selector).collect::<Vec<_>>();
        if page_links.is_empty() {
            return result;
        }

        if let Some(last_page_text) = page_links
            .last()
            .map(|e| e.text().collect::<String>().trim().to_string())
            && let Ok(last_page_number) = last_page_text.parse::<usize>()
            && let Ok(base_url) = self.base_url.join(url)
        {
            for i in 2..=last_page_number {
                // Clone the URL and filter out the old `page` parameter
                let mut page_url = base_url.clone();
                let mut pairs: Vec<(String, String)> = page_url
                    .query_pairs()
                    .filter(|(k, _)| k != "PAGEN_1")
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                // Insert the new page value
                pairs.push(("PAGEN_1".to_string(), i.to_string()));

                // Clear existing query and re-apply
                page_url.set_query(None);
                page_url
                    .query_pairs_mut()
                    .extend_pairs(pairs.iter().map(|(k, v)| (&**k, &**v)));

                result.push(page_url.to_string());
            }
        }

        result
    }

    /// Extracts product detail links from a listing page.
    async fn get_product_links(&self, url: &str) -> Vec<String> {
        let document = match self.fetch_html(url).await {
            Some(doc) => doc,
            None => {
                log::error!("Failed to parse HTML {url}");
                return vec![];
            }
        };

        let selector = Selector::parse("div.product-card__info-bottom > a").unwrap();
        document
            .select(&selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                Some(self.base_url.join(href).ok()?.to_string())
            })
            .collect()
    }
}

#[async_trait]
impl WebstoreCrawler for WebstoreCrawler101Tea {
    /// Crawls the entire web store and returns all discovered products.
    ///
    /// Category pages, pagination, product links and product details are
    /// fetched concurrently with `join_all`, while [`fetch_html`] ensures the
    /// number of simultaneous HTTP requests never exceeds the configured
    /// limit.
    async fn get_products(&self) -> Vec<NewProduct> {
        let categories = self.get_category_links().await;

        let mut tasks = vec![];
        for category in categories.iter() {
            tasks.push(async { self.get_page_links(category).await });
        }
        let page_links = futures::future::join_all(tasks).await;

        let mut tasks = vec![];
        for page_link in page_links.iter().flatten() {
            tasks.push(async { self.get_product_links(page_link).await });
        }
        let product_links = futures::future::join_all(tasks).await;

        // Deduplicate product links to avoid fetching the same page multiple times.
        let unique_links: HashSet<String> = product_links.into_iter().flatten().collect();

        let mut tasks = vec![];
        for link in &unique_links {
            tasks.push(async { self.get_product(link).await });
        }
        let products = futures::future::join_all(tasks).await;

        // Flatten and ensure uniqueness by product URL in the final result.
        let mut products: Vec<NewProduct> = products.into_iter().flatten().collect();
        let mut seen_urls = HashSet::new();
        products.retain(|p| seen_urls.insert(p.url.clone()));
        products
    }

    /// Fetches product information from a single product page.
    ///
    /// A page may describe multiple variants; each variant is converted into
    /// its own [`NewProduct`].
    async fn get_product(&self, url: &str) -> Vec<NewProduct> {
        let document = match self.fetch_html(url).await {
            Some(doc) => doc,
            None => {
                log::error!("Failed to parse HTML {url}");
                return vec![];
            }
        };

        // Name
        let name_selector = Selector::parse("h1").unwrap();
        let name = document
            .select(&name_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Description
        let desc_selector =
            Selector::parse("div.catalog-table_content-item_about_product").unwrap();
        let description = document
            .select(&desc_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Category from breadcrumbs
        let category_selector = Selector::parse("a.breadcrumbs__list-link").unwrap();
        let category = document
            .select(&category_selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect::<Vec<_>>()
            .join(" / ");

        // Price
        let price_selector = Selector::parse("span.js-price-val").unwrap();
        let price = document
            .select(&price_selector)
            .next()
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .trim()
                    .to_string()
                    .replace(",", ".")
                    .replace(" ", "")
            })
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or_default();

        // SKU
        let sku_selector = Selector::parse("div.product_art span:nth-child(2)").unwrap();
        let sku = document
            .select(&sku_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Units
        let units_selector = Selector::parse("span.product-card__calculus-unit").unwrap();
        let units = document
            .select(&units_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Amount
        let amount_selector = Selector::parse("span.js-product-calc-value").unwrap();
        let amount = document
            .select(&amount_selector)
            .next()
            .map(|el| {
                el.text()
                    .collect::<String>()
                    .trim()
                    .to_string()
                    .replace(",", ".")
                    .replace(" ", "")
            })
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or_default();

        build_new_product(
            self.crawler_id,
            sku,
            name,
            Some(category),
            Some(units),
            price,
            Some(amount),
            Some(description),
            url.to_string(),
            vec![],
        )
        .into_iter()
        .collect()
    }
}
