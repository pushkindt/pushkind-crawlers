use pushkind_common::db::{DbConnection, DbPool};
use pushkind_common::repository::errors::RepositoryResult;
use pushkind_dantes::domain::benchmark::Benchmark;
use pushkind_dantes::domain::crawler::Crawler;
use pushkind_dantes::domain::product::{NewProduct, Product};
use pushkind_dantes::domain::types::{
    BenchmarkId, CrawlerId, CrawlerSelectorValue, HubId, ProductId, SimilarityDistance,
};

pub mod benchmark;
pub mod crawler;
pub mod product;

/// Diesel-backed repository implementation using a connection pool.
pub struct DieselRepository {
    /// Shared database pool used to obtain connections.
    pool: DbPool,
}

impl DieselRepository {
    /// Construct a new repository backed by the provided pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn conn(&self) -> RepositoryResult<DbConnection> {
        Ok(self.pool.get()?)
    }
}

/// Defines read-only operations for accessing products.
pub trait ProductReader {
    fn list_products(&self, crawler_id: CrawlerId) -> RepositoryResult<Vec<Product>>;
}

/// Defines write operations for storing and mutating products.
pub trait ProductWriter {
    fn create_products(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn update_products(&self, products: &[NewProduct]) -> RepositoryResult<usize>;
    fn set_product_embedding(
        &self,
        product_id: ProductId,
        embedding: &[f32],
    ) -> RepositoryResult<usize>;
    fn delete_products(&self, crawler_id: CrawlerId) -> RepositoryResult<usize>;
}

/// Retrieves a single crawler from the repository.
pub trait CrawlerReader {
    fn get_crawler(&self, selector: &CrawlerSelectorValue) -> RepositoryResult<Crawler>;
    fn list_crawlers(&self, hub_id: HubId) -> RepositoryResult<Vec<Crawler>>;
}

/// Persists changes to crawler records.
pub trait CrawlerWriter {
    fn update_crawler_stats(&self, crawler_id: CrawlerId) -> RepositoryResult<usize>;
    fn set_crawler_processing(
        &self,
        crawler_id: CrawlerId,
        processing: bool,
    ) -> RepositoryResult<usize>;
}

/// Provides read access to benchmark metadata.
pub trait BenchmarkReader {
    fn get_benchmark(&self, benchmark_id: BenchmarkId) -> RepositoryResult<Benchmark>;
}

/// Provides methods to mutate benchmark records and their associations.
pub trait BenchmarkWriter {
    fn set_benchmark_embedding(
        &self,
        benchmark_id: BenchmarkId,
        embedding: &[f32],
    ) -> RepositoryResult<usize>;
    fn set_benchmark_association(
        &self,
        benchmark_id: BenchmarkId,
        product_id: ProductId,
        distance: SimilarityDistance,
    ) -> RepositoryResult<usize>;
    fn remove_benchmark_associations(&self, benchmark_id: BenchmarkId) -> RepositoryResult<usize>;
    fn set_benchmark_processing(
        &self,
        benchmark_id: BenchmarkId,
        processing: bool,
    ) -> RepositoryResult<usize>;
    fn update_benchmark_stats(&self, benchmark_id: BenchmarkId) -> RepositoryResult<usize>;
}
