use pushkind_common::domain::benchmark::Benchmark;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::domain::product::{NewProduct, Product};
use pushkind_common::repository::errors::RepositoryResult;

pub mod benchmark;
pub mod crawler;
pub mod product;

/// Defines read-only operations for accessing products.
pub trait ProductReader {
    fn list(&self, crawler_id: i32) -> RepositoryResult<Vec<Product>>;
}

/// Defines write operations for storing and mutating products.
pub trait ProductWriter {
    fn create(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn update(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn set_embedding(&self, product_id: i32, embedding: &[f32]) -> RepositoryResult<usize>;
    fn delete(&self, crawler_id: i32) -> RepositoryResult<usize>;
}

/// Retrieves a single crawler from the repository.
pub trait CrawlerReader {
    fn get(&self, selector: &str) -> RepositoryResult<Crawler>;
}

/// Persists changes to crawler records.
pub trait CrawlerWriter {
    fn update(&self, crawler_id: i32) -> RepositoryResult<usize>;
    fn set_processing(&self, crawler_id: i32, processing: bool) -> RepositoryResult<usize>;
}

/// Provides read access to benchmark metadata.
pub trait BenchmarkReader {
    fn get(&self, benchmark_id: i32) -> RepositoryResult<Benchmark>;
}

/// Provides methods to mutate benchmark records and their associations.
pub trait BenchmarkWriter {
    fn set_embedding(&self, benchmark_id: i32, embedding: &[f32]) -> RepositoryResult<usize>;
    fn set_association(
        &self,
        benchmark_id: i32,
        product_id: i32,
        distance: f32,
    ) -> RepositoryResult<usize>;
    fn remove_associations(&self, benchmark_id: i32) -> RepositoryResult<usize>;
    fn set_processing(&self, benchmark_id: i32, processing: bool) -> RepositoryResult<usize>;
}
