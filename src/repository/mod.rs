use pushkind_common::db::DbPool;
use pushkind_common::domain::benchmark::Benchmark;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::domain::product::{NewProduct, Product};
use pushkind_common::repository::errors::RepositoryResult;

pub mod benchmark;
pub mod crawler;
pub mod product;

/// Diesel-backed repository implementation using a connection pool.
pub struct DieselRepository<'a> {
    /// Shared database pool used to obtain connections.
    pub pool: &'a DbPool,
}

impl<'a> DieselRepository<'a> {
    /// Construct a new repository backed by the provided pool.
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }
}

/// Defines read-only operations for accessing products.
pub trait ProductReader {
    fn list_products(&self, crawler_id: i32) -> RepositoryResult<Vec<Product>>;
}

/// Defines write operations for storing and mutating products.
pub trait ProductWriter {
    fn create_products(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn update_products(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn set_product_embedding(&self, product_id: i32, embedding: &[f32]) -> RepositoryResult<usize>;
    fn delete_products(&self, crawler_id: i32) -> RepositoryResult<usize>;
}

/// Retrieves a single crawler from the repository.
pub trait CrawlerReader {
    fn get_crawler(&self, selector: &str) -> RepositoryResult<Crawler>;
    fn list_crawlers(&self, hub_id: i32) -> RepositoryResult<Vec<Crawler>>;
}

/// Persists changes to crawler records.
pub trait CrawlerWriter {
    fn update_crawler_stats(&self, crawler_id: i32) -> RepositoryResult<usize>;
    fn set_crawler_processing(&self, crawler_id: i32, processing: bool) -> RepositoryResult<usize>;
}

/// Provides read access to benchmark metadata.
pub trait BenchmarkReader {
    fn get_benchmark(&self, benchmark_id: i32) -> RepositoryResult<Benchmark>;
}

/// Provides methods to mutate benchmark records and their associations.
pub trait BenchmarkWriter {
    fn set_benchmark_embedding(
        &self,
        benchmark_id: i32,
        embedding: &[f32],
    ) -> RepositoryResult<usize>;
    fn set_benchmark_association(
        &self,
        benchmark_id: i32,
        product_id: i32,
        distance: f32,
    ) -> RepositoryResult<usize>;
    fn remove_benchmark_associations(&self, benchmark_id: i32) -> RepositoryResult<usize>;
    fn set_benchmark_processing(
        &self,
        benchmark_id: i32,
        processing: bool,
    ) -> RepositoryResult<usize>;
}
