use pushkind_common::domain::crawler::Crawler;
use pushkind_common::domain::product::NewProduct;
use pushkind_common::repository::errors::RepositoryResult;

pub mod crawler;
pub mod product;

pub trait ProductWriter {
    fn create(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn update(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn delete(&self, crawler_id: i32) -> RepositoryResult<usize>;
}

pub trait CrawlerReader {
    fn get(&self, selector: &str) -> RepositoryResult<Crawler>;
}
