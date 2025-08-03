use pushkind_common::domain::benchmark::Benchmark;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::domain::product::{NewProduct, Product};
use pushkind_common::repository::errors::RepositoryResult;

pub mod crawler;
pub mod product;

pub trait ProductReader {
    fn list(&self, crawler_id: i32) -> RepositoryResult<Vec<Product>>;
}

pub trait ProductWriter {
    fn create(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn update(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn set_embedding(&self, product_id: i32, embedding: &[f32]) -> RepositoryResult<usize>;
    fn delete(&self, crawler_id: i32) -> RepositoryResult<usize>;
}

pub trait CrawlerReader {
    fn get(&self, selector: &str) -> RepositoryResult<Crawler>;
}

pub trait CrawlerWriter {
    fn update(&self, crawler_id: i32) -> RepositoryResult<usize>;
}

pub trait BenchmarkReader {
    fn get(&self, benchmark_id: i32) -> RepositoryResult<Benchmark>;
}

pub trait BenchmarkWriter {
    fn set_embedding(&self, benchmark_id: i32, embedding: &[f32]) -> RepositoryResult<usize>;
    fn set_association(&self, benchmark_id: i32, product_id: i32) -> RepositoryResult<usize>;
    fn remove_associations(&self, benchmark_id: i32) -> RepositoryResult<usize>;
}
