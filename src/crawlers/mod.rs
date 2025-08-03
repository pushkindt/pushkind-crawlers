use crate::domain::product::Product;

pub mod rusteaco;
pub mod tea101;

pub trait Crawler {
    async fn get_products(&self) -> Vec<Product>;
    async fn get_product(&self, url: &str) -> Vec<Product>;
}
