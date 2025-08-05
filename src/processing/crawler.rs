use futures::future;
use pushkind_common::models::zmq::dantes::CrawlerSelector;

use crate::crawlers::Crawler;
use crate::crawlers::rusteaco::WebstoreCrawlerRusteaco;
use crate::repository::CrawlerReader;
use crate::repository::CrawlerWriter;
use crate::repository::ProductWriter;

/// Processes a message for a specific crawler and either refreshes all of its
/// products or updates a subset. When no product URLs are provided, existing
/// items are cleared and the crawler fetches all products anew. If URLs are
/// supplied, only those products are retrieved and updated in the repository.
pub async fn process_crawler_message<R>(msg: CrawlerSelector, repo: R)
where
    R: CrawlerReader + CrawlerWriter + ProductWriter,
{
    log::info!("Received crawler: {msg:?}");

    let (selector, urls) = match msg {
        CrawlerSelector::Selector(selector) => (selector, vec![]),
        CrawlerSelector::SelectorProducts((selector, urls)) => (selector, urls),
    };

    let crawler = match repo.get_crawler(&selector) {
        Ok(crawler) => crawler,
        Err(e) => {
            log::error!("Error retrieving selector: {e}");
            return;
        }
    };

    if crawler.processing {
        log::warn!("Crawler {selector} is already running");
        return;
    }

    if let Err(e) = repo.set_crawler_processing(crawler.id, true) {
        log::error!("Failed to set benchmark processing: {e:?}");
    }

    if selector == "rusteaco" {
        let rusteaco = WebstoreCrawlerRusteaco::new(5, crawler.id);
        if urls.is_empty() {
            if let Err(e) = repo.delete_products(crawler.id) {
                log::error!("Error deleting products: {e}");
                return;
            }
            let products = rusteaco.get_products().await;
            if let Err(e) = repo.create_products(&products) {
                log::error!("Error creating products: {e}");
            }
        } else {
            let tasks = urls.into_iter().map(|url| {
                let crawler = &rusteaco;
                async move { crawler.get_product(&url).await }
            });
            let products = future::join_all(tasks)
                .await
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            if let Err(e) = repo.update_products(&products) {
                log::error!("Error updating products: {e}");
            }
        }
    } else {
        log::error!("Unknown crawler: {selector}");
    }

    if let Err(e) = repo.update_crawler_stats(crawler.id) {
        log::error!("Error updating crawler stats: {e}");
    }

    log::info!("Finished processing crawler: {selector}");
}
