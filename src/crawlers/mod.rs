use async_trait::async_trait;
use pushkind_common::domain::product::NewProduct;

pub mod rusteaco;
pub mod tea101;

#[async_trait]
pub trait Crawler {
    async fn get_products(&self) -> Vec<NewProduct>;
    async fn get_product(&self, url: &str) -> Vec<NewProduct>;
}
