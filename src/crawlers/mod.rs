use crate::domain::product::Product;
use async_trait::async_trait;

pub mod rusteaco;
pub mod tea101;

#[async_trait]
pub trait Crawler {
    async fn get_products(&self) -> Vec<Product>;
    async fn get_product(&self, url: &str) -> Vec<Product>;
}
