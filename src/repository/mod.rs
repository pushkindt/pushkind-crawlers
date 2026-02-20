use pushkind_common::db::{DbConnection, DbPool};
use pushkind_common::repository::errors::RepositoryResult;
use pushkind_dantes::domain::benchmark::Benchmark;
use pushkind_dantes::domain::category::Category;
use pushkind_dantes::domain::crawler::Crawler;
use pushkind_dantes::domain::product::{NewProduct, Product};
use pushkind_dantes::domain::types::{
    BenchmarkId, CategoryId, CrawlerId, CrawlerSelectorValue, HubId, ProductId, SimilarityDistance,
};

pub mod benchmark;
pub mod category;
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

/// Provides read access to canonical category records.
pub trait CategoryReader {
    fn list_categories(&self, hub_id: HubId) -> RepositoryResult<Vec<Category>>;
}

/// Provides methods to mutate category records.
pub trait CategoryWriter {
    fn set_category_embedding(
        &self,
        category_id: CategoryId,
        embedding: &[f32],
    ) -> RepositoryResult<usize>;
}

/// Provides methods to update product-to-category assignments.
pub trait ProductCategoryWriter {
    /// Set an automatic category assignment for a product.
    fn set_product_category_automatic(
        &self,
        product_id: ProductId,
        category_id: Option<CategoryId>,
    ) -> RepositoryResult<usize>;

    /// Clear category assignments for all products under a crawler.
    fn clear_product_categories_by_crawler(&self, crawler_id: CrawlerId)
    -> RepositoryResult<usize>;
}

/// Provides read methods for hub-scoped processing guard checks.
pub trait ProcessingGuardReader {
    /// Returns `true` if any crawler or benchmark in the hub is marked as processing.
    fn has_any_processing_in_hub(&self, hub_id: HubId) -> RepositoryResult<bool>;
}

/// Provides write methods for hub-scoped processing guard state transitions.
pub trait ProcessingGuardWriter {
    /// Atomically claim hub processing lock by setting crawler/benchmark
    /// processing flags to `true` when none are currently active.
    ///
    /// Returns `Ok(true)` when lock is claimed, `Ok(false)` when already held.
    fn claim_hub_processing_lock(&self, hub_id: HubId) -> RepositoryResult<bool>;

    /// Release hub processing lock by setting crawler/benchmark processing flags
    /// to `false`.
    fn release_hub_processing_lock(&self, hub_id: HubId) -> RepositoryResult<usize>;

    fn set_hub_crawlers_processing(
        &self,
        hub_id: HubId,
        processing: bool,
    ) -> RepositoryResult<usize>;
    fn set_hub_benchmarks_processing(
        &self,
        hub_id: HubId,
        processing: bool,
    ) -> RepositoryResult<usize>;
}
