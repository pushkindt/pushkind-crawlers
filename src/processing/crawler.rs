use futures::future;
use pushkind_common::db::DbPool;

use crate::crawlers::Crawler;
use crate::crawlers::rusteaco::WebstoreCrawlerRusteaco;
use crate::processing::CrawlerSelector;
use crate::repository::CrawlerReader;
use crate::repository::CrawlerWriter;
use crate::repository::ProductWriter;
use crate::repository::crawler::DieselCrawlerRepository;
use crate::repository::product::DieselProductRepository;

pub async fn process_crawler_message(msg: CrawlerSelector, db_pool: &DbPool) {
    log::info!("Received crawler: {msg:?}");
    let product_repo = DieselProductRepository::new(db_pool);
    let crawler_repo = DieselCrawlerRepository::new(db_pool);

    let (selector, urls) = match msg {
        CrawlerSelector::Selector(selector) => (selector, vec![]),
        CrawlerSelector::SelectorProducts((selector, urls)) => (selector, urls),
    };

    let crawler = match crawler_repo.get(&selector) {
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

    if let Err(e) = crawler_repo.set_processing(crawler.id, true) {
        log::error!("Failed to set benchmark processing: {e:?}");
    }

    if selector == "rusteaco" {
        let rusteaco = WebstoreCrawlerRusteaco::new(5, crawler.id);
        if urls.is_empty() {
            if let Err(e) = product_repo.delete(crawler.id) {
                log::error!("Error deleting products: {e}");
                return;
            }
            let products = rusteaco.get_products().await;
            if let Err(e) = product_repo.create(&products) {
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
            if let Err(e) = product_repo.update(&products) {
                log::error!("Error updating products: {e}");
            }
        }
    } else {
        log::error!("Unknown crawler: {selector}");
    }

    if let Err(e) = crawler_repo.update(crawler.id) {
        log::error!("Error updating crawler stats: {e}");
    }

    log::info!("Finished processing crawler: {selector}");
}
