use diesel::prelude::*;
use pushkind_common::db::DbPool;
use pushkind_common::domain::crawler::Crawler;
use pushkind_common::models::crawler::Crawler as DbCrawler;
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::CrawlerReader;

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

        let result = crawlers::table
            .filter(crawlers::selector.eq(selector))
            .first::<DbCrawler>(&mut conn)?;

        Ok(result.into()) // Convert DbCrawler to DomainCrawler
    }
}
