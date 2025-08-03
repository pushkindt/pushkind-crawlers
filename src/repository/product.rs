use diesel::prelude::*;
use pushkind_common::db::DbPool;
use pushkind_common::domain::product::NewProduct;
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::ProductWriter;

pub struct DieselProductRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> DieselProductRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }
}

impl ProductWriter for DieselProductRepository<'_> {
    fn create(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        Ok(0)
    }

    fn update(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        Ok(0)
    }

    fn delete(&self, crawler_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::{product_benchmark, products};

        let mut conn = self.pool.get()?;

        Ok(0)
    }
}
