use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use html_escape::decode_html_entities;
use pushkind_dantes::domain::product::NewProduct;
use scraper::{Html, Selector};
use serde::Deserialize;
use tokio::sync::Semaphore;
use url::Url;

use crate::crawlers::{
    CrawlerError, CrawlerResult, WebstoreCrawler, build_new_product, build_reqwest_client,
    parse_amount_units,
};

#[derive(Debug, Deserialize, Clone)]
struct Variant {
    sku: String,
    price: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct ProductJson {
    variants: Vec<Variant>,
}

/// Converts a [`Variant`] produced by the store into a [`NewProduct`].
fn variant_to_product(
    v: Variant,
    name: &str,
    category: &str,
    description: &str,
    url: &str,
    crawler_id: i32,
) -> Option<NewProduct> {
    let (amount, units) = parse_amount_units(&v.title);
    let price = v.price.replace(',', ".").parse().unwrap_or(0.0);

    build_new_product(
        crawler_id,
        v.sku.clone(),
        name.to_string(),
        Some(category.to_string()),
        Some(units),
        price,
        Some(amount),
        Some(description.to_string()),
        format!("{url}#{}", v.sku),
        vec![],
    )
}

/// Crawler for `shop.rusteaco.ru` which limits concurrent HTTP requests
/// using a [`Semaphore`].
pub struct WebstoreCrawlerRusteaco {
    crawler_id: i32,
    base_url: Url,
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
}

impl WebstoreCrawlerRusteaco {
    /// Creates a new crawler with the given concurrency limit.
    ///
    /// `concurrency` controls how many HTTP requests may be in flight at the
    /// same time. The `crawler_id` is attached to each produced product.
    pub fn new(concurrency: usize, crawler_id: i32) -> CrawlerResult<Self> {
        Ok(Self {
            crawler_id,
            base_url: Url::parse("https://shop.rusteaco.ru/")
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

        let selector = Selector::parse("a.header__collections-link").unwrap();

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

        let selector = Selector::parse("div.pagination-items").unwrap();
        let pagination = match document.select(&selector).next() {
            Some(p) => p,
            None => return result,
        };

        let selector = Selector::parse("a.pagination-link").unwrap();
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
                    .filter(|(k, _)| k != "page")
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                // Insert the new page value
                pairs.push(("page".to_string(), i.to_string()));

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

        let selector = Selector::parse("div.product-preview__title > a").unwrap();
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
impl WebstoreCrawler for WebstoreCrawlerRusteaco {
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
        let name_selector = Selector::parse("h1.product__title").unwrap();
        let name = document
            .select(&name_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Description
        let desc_selector = Selector::parse("div.product__short-description").unwrap();
        let description = document
            .select(&desc_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Category from breadcrumbs
        let category_selector = Selector::parse("ul.breadcrumb li a").unwrap();
        let category = document
            .select(&category_selector)
            .map(|el| el.text().collect::<String>().trim().to_string())
            .collect::<Vec<_>>()
            .join(" / ");

        let selector = Selector::parse("form.product").unwrap();
        let Some(product_form) = document.select(&selector).next() else {
            log::error!("Failed to find form.product {url}");
            return vec![];
        };

        if let Some(json_raw) = product_form.value().attr("data-product-json") {
            // Convert HTML-encoded string to valid JSON
            let json_str = decode_html_entities(json_raw).to_string();
            // Now parse it
            let parsed: ProductJson = match serde_json::from_str(&json_str) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("Failed to parse product JSON {url}: {e}");
                    return vec![];
                }
            };

            parsed
                .variants
                .into_iter()
                .filter_map(|v| {
                    variant_to_product(v, &name, &category, &description, url, self.crawler_id)
                })
                .collect()
        } else {
            // SKU
            let sku_selector = Selector::parse("span.sku-value").unwrap();
            let sku = document
                .select(&sku_selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            // Amount and units are a string like "150 г"
            let amount_units_selector = Selector::parse("button.option-value").unwrap();
            let amount_units = document
                .select(&amount_units_selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let (amount, units) = parse_amount_units(&amount_units);

            // Price
            let price_selector = Selector::parse("span.product__price-cur").unwrap();
            let price = document
                .select(&price_selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let price = price
                .replace(',', ".")
                .replace(" ", "")
                .parse()
                .unwrap_or(0.0);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_product_fields() -> (&'static str, &'static str, &'static str, &'static str) {
        ("Name", "Category", "Description", "http://example.com")
    }

    #[test]
    fn converts_weight_to_kg() {
        let variant = Variant {
            sku: "S1".into(),
            price: "10,5".into(),
            title: "0.5 кг".into(),
        };
        let (name, category, description, url) = dummy_product_fields();
        let product = variant_to_product(variant, name, category, description, url, 1).unwrap();
        assert_eq!(product.units.as_deref(), Some("кг"));
        assert!((product.amount.unwrap().get() - 0.5).abs() < f64::EPSILON);
        assert!((product.price.get() - 10.5).abs() < f64::EPSILON);
    }

    #[test]
    fn defaults_to_pieces_when_weight_missing() {
        let variant = Variant {
            sku: "S2".into(),
            price: "20".into(),
            title: "".into(),
        };
        let (name, category, description, url) = dummy_product_fields();
        let product = variant_to_product(variant, name, category, description, url, 1).unwrap();
        assert_eq!(product.units.as_deref(), Some("шт"));
        assert!((product.amount.unwrap().get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn defaults_to_pieces_when_weight_invalid() {
        let variant = Variant {
            sku: "S3".into(),
            price: "15".into(),
            title: "abc".into(),
        };
        let (name, category, description, url) = dummy_product_fields();
        let product = variant_to_product(variant, name, category, description, url, 1).unwrap();
        assert_eq!(product.units.as_deref(), Some("шт"));
        assert!((product.amount.unwrap().get() - 1.0).abs() < f64::EPSILON);
    }
}
