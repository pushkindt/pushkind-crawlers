use async_trait::async_trait;
use pushkind_common::domain::product::NewProduct;

pub mod rusteaco;
pub mod tea101;

/// An abstraction over web store crawlers that produce [`NewProduct`]s.
#[async_trait]
pub trait WebstoreCrawler: Send + Sync {
    /// Crawls the target site and returns every product discovered.
    async fn get_products(&self) -> Vec<NewProduct>;

    /// Fetches product information from a single URL.
    ///
    /// Some pages may describe multiple product variants, therefore the
    /// implementation returns a collection of [`NewProduct`]s.
    async fn get_product(&self, url: &str) -> Vec<NewProduct>;
}
