use diesel::prelude::*;
use pushkind_common::db::DbPool;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::models::crawler::Crawler as DbCrawler;
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::{CrawlerReader, CrawlerWriter};

/// Repository for crawler records using Diesel and PostgreSQL.
pub struct DieselCrawlerRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> DieselCrawlerRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }
}

impl CrawlerReader for DieselCrawlerRepository<'_> {
    fn get(&self, selector: &str) -> RepositoryResult<Crawler> {
        use pushkind_common::schema::dantes::crawlers;

        let mut conn = self.pool.get()?;

        // Query the crawler by its unique selector
        let result = crawlers::table
            .filter(crawlers::selector.eq(selector))
            .first::<DbCrawler>(&mut conn)?;

        Ok(result.into()) // Convert DbCrawler to DomainCrawler
    }
}

impl CrawlerWriter for DieselCrawlerRepository<'_> {
    fn update(&self, crawler_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::crawlers;
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        // Count products for the crawler to update statistics
        let product_count: i64 = products::table
            .filter(products::crawler_id.eq(crawler_id))
            .count()
            .get_result(&mut conn)?;

        // Update timestamp, processing state and product count
        let result = diesel::update(crawlers::table.filter(crawlers::id.eq(crawler_id)))
            .set((
                crawlers::updated_at.eq(diesel::dsl::now),
                crawlers::processing.eq(false),
                crawlers::num_products.eq(product_count as i32), // cast if needed
            ))
            .execute(&mut conn)?;

        Ok(result)
    }

    fn set_processing(&self, crawler_id: i32, processing: bool) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::crawlers;

        let mut conn = self.pool.get()?;

        let affected = diesel::update(crawlers::table.filter(crawlers::id.eq(crawler_id)))
            .set(crawlers::processing.eq(processing))
            .execute(&mut conn)?;

        Ok(affected)
    }
}
