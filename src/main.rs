use std::fs::File;
use std::io::Write;

use serde_json;

use pushkind_crawlers::crawlers::Crawler;
use pushkind_crawlers::crawlers::rusteaco::WebstoreCrawlerRusteaco;
use pushkind_crawlers::domain::product::Product;

fn save_products_as_json(products: &[Product], path: &str) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(products)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let rusteaco = WebstoreCrawlerRusteaco::new(5);

    let products = rusteaco.get_products().await;

    if let Err(e) = save_products_as_json(&products, "products.json") {
        log::error!("Failed to save products: {e}");
    }
}
