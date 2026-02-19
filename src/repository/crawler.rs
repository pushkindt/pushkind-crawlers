use diesel::prelude::*;
use pushkind_common::repository::errors::{RepositoryError, RepositoryResult};
use pushkind_dantes::domain::crawler::Crawler;
use pushkind_dantes::domain::types::{CrawlerId, CrawlerSelectorValue, HubId};
use pushkind_dantes::models::crawler::Crawler as DbCrawler;

use crate::repository::{CrawlerReader, CrawlerWriter, DieselRepository};

impl CrawlerReader for DieselRepository {
    fn get_crawler(&self, selector: &CrawlerSelectorValue) -> RepositoryResult<Crawler> {
        use pushkind_dantes::schema::crawlers;

        let mut conn = self.conn()?;

        // Query the crawler by its unique selector
        let result = crawlers::table
            .filter(crawlers::selector.eq(selector.as_str()))
            .first::<DbCrawler>(&mut conn)?;

        Crawler::try_from(result).map_err(|err| RepositoryError::ValidationError(err.to_string()))
    }

    fn list_crawlers(&self, hub_id: HubId) -> RepositoryResult<Vec<Crawler>> {
        use pushkind_dantes::schema::crawlers;

        let mut conn = self.conn()?;

        let result = crawlers::table
            .filter(crawlers::hub_id.eq(hub_id.get()))
            .load::<DbCrawler>(&mut conn)?;

        result
            .into_iter()
            .map(Crawler::try_from)
            .collect::<Result<Vec<Crawler>, _>>()
            .map_err(|err| RepositoryError::ValidationError(err.to_string()))
    }
}

impl CrawlerWriter for DieselRepository {
    fn update_crawler_stats(&self, crawler_id: CrawlerId) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::crawlers;
        use pushkind_dantes::schema::products;

        let mut conn = self.conn()?;

        // Count products for the crawler to update statistics
        let product_count: i64 = products::table
            .filter(products::crawler_id.eq(crawler_id.get()))
            .count()
            .get_result(&mut conn)?;

        // Update timestamp, processing state and product count
        let result = diesel::update(crawlers::table.filter(crawlers::id.eq(crawler_id.get())))
            .set((
                crawlers::updated_at.eq(diesel::dsl::now),
                crawlers::processing.eq(false),
                crawlers::num_products.eq(product_count as i32), // cast if needed
            ))
            .execute(&mut conn)?;

        Ok(result)
    }

    fn set_crawler_processing(
        &self,
        crawler_id: CrawlerId,
        processing: bool,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::crawlers;

        let mut conn = self.conn()?;

        let affected = diesel::update(crawlers::table.filter(crawlers::id.eq(crawler_id.get())))
            .set(crawlers::processing.eq(processing))
            .execute(&mut conn)?;

        Ok(affected)
    }
}
