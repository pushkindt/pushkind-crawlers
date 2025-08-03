use std::env;
use std::sync::Arc;

use futures::future;
use pushkind_common::db::DbPool;
use pushkind_common::db::establish_connection_pool;
use serde::Deserialize;

use pushkind_crawlers::crawlers::Crawler;
use pushkind_crawlers::crawlers::rusteaco::WebstoreCrawlerRusteaco;
use pushkind_crawlers::repository::CrawlerReader;
use pushkind_crawlers::repository::ProductWriter;
use pushkind_crawlers::repository::crawler::DieselCrawlerRepository;
use pushkind_crawlers::repository::product::DieselProductRepository;

#[derive(Deserialize, Debug)]
enum ZMQMessage {
    CrawlerSelector(String),
    ProductURLs((String, Vec<String>)),
}

async fn proccess_zmq_message(msg: ZMQMessage, db_pool: &DbPool) {
    log::info!("Received: {msg:?}");
    let product_repo = DieselProductRepository::new(db_pool);
    let crawler_repo = DieselCrawlerRepository::new(db_pool);

    let (selector, urls) = match msg {
        ZMQMessage::CrawlerSelector(selector) => (selector, vec![]),
        ZMQMessage::ProductURLs((selector, urls)) => (selector, urls),
    };

    let crawler = match crawler_repo.get(&selector) {
        Ok(crawler) => crawler,
        Err(e) => {
            log::error!("Error retrieving selector: {e}");
            return;
        }
    };

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
    }
    log::info!("Finished processing: {msg:?}");
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let database_url = env::var("DATABASE_URL").unwrap_or("app.db".to_string());
    let pool = match establish_connection_pool(&database_url) {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("Failed to establish database connection: {e}");
            std::process::exit(1);
        }
    };
    let pool = Arc::new(pool);

    let zmq_address =
        env::var("ZMQ_ADDRESS").unwrap_or_else(|_| "tcp://127.0.0.1:5555".to_string());
    let context = zmq::Context::new();
    let responder = context.socket(zmq::PULL).expect("Cannot create zmq socket");
    responder
        .bind(&zmq_address)
        .expect("Cannot bind to zmq port");

    loop {
        let msg = responder.recv_bytes(0).unwrap();
        match serde_json::from_slice::<ZMQMessage>(&msg) {
            Ok(parsed) => {
                let pool_clone = Arc::clone(&pool);
                tokio::spawn(async move { proccess_zmq_message(parsed, &pool_clone).await });
            }
            Err(e) => log::error!("Failed to parse JSON: {e}"),
        }
    }
}
