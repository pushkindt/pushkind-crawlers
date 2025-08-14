use diesel::prelude::*;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::models::crawler::Crawler as DbCrawler;
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::{CrawlerReader, CrawlerWriter, DieselRepository};

impl CrawlerReader for DieselRepository {
    fn get_crawler(&self, selector: &str) -> RepositoryResult<Crawler> {
        use pushkind_common::schema::dantes::crawlers;

        let mut conn = self.conn()?;

        // Query the crawler by its unique selector
        let result = crawlers::table
            .filter(crawlers::selector.eq(selector))
            .first::<DbCrawler>(&mut conn)?;

        Ok(result.into()) // Convert DbCrawler to DomainCrawler
    }

    fn list_crawlers(&self, hub_id: i32) -> RepositoryResult<Vec<Crawler>> {
        use pushkind_common::schema::dantes::crawlers;

        let mut conn = self.conn()?;

        let result = crawlers::table
            .filter(crawlers::hub_id.eq(hub_id))
            .load::<DbCrawler>(&mut conn)?;

        Ok(result.into_iter().map(|c| c.into()).collect())
    }
}

impl CrawlerWriter for DieselRepository {
    fn update_crawler_stats(&self, crawler_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::crawlers;
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

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

    fn set_crawler_processing(&self, crawler_id: i32, processing: bool) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::crawlers;

        let mut conn = self.conn()?;

        let affected = diesel::update(crawlers::table.filter(crawlers::id.eq(crawler_id)))
            .set(crawlers::processing.eq(processing))
            .execute(&mut conn)?;

        Ok(affected)
    }
}
