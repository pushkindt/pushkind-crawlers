use bytemuck::cast_slice;
use chrono::Utc;
use diesel::prelude::*;
use pushkind_common::db::DbPool;
use pushkind_common::domain::product::{NewProduct, Product};
use pushkind_common::models::product::{NewProduct as DbNewProduct, Product as DbProduct};
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::ProductReader;
use crate::repository::ProductWriter;

pub struct DieselProductRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> DieselProductRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }
}

impl ProductReader for DieselProductRepository<'_> {
    fn list(&self, hub_id: i32) -> RepositoryResult<Vec<Product>> {
        use pushkind_common::schema::dantes::{crawlers, products};

        let mut conn = self.pool.get()?;

        let products: Vec<DbProduct> = products::table
            .inner_join(crawlers::table)
            .filter(crawlers::hub_id.eq(hub_id))
            .select(products::all_columns)
            .load::<DbProduct>(&mut conn)?;

        Ok(products.into_iter().map(Into::into).collect())
    }
}

impl ProductWriter for DieselProductRepository<'_> {
    fn create(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        let new_products: Vec<DbNewProduct> = products.iter().cloned().map(Into::into).collect();

        let inserted = diesel::insert_into(products::table)
            .values(&new_products)
            .execute(&mut conn)?;

        Ok(inserted)
    }

    fn update(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        let mut affected_rows = 0;
        for product in products.iter().cloned() {
            let db_product: DbNewProduct = product.into();
            let rows = diesel::insert_into(products::table)
                .values(&db_product)
                .on_conflict((products::crawler_id, products::url))
                .do_update()
                .set((&db_product, products::updated_at.eq(Utc::now().naive_utc())))
                .execute(&mut conn)?;
            affected_rows += rows;
        }

        Ok(affected_rows)
    }

    fn set_embedding(&self, product_id: i32, embedding: &[f32]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.pool.get()?;

        // Convert &[f32] to &[u8]
        let blob: Vec<u8> = cast_slice(embedding).to_vec();

        let affected = diesel::update(products::table.filter(products::id.eq(product_id)))
            .set(products::embedding.eq(blob))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn delete(&self, crawler_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::{product_benchmark, products};

        let mut conn = self.pool.get()?;

        let deleted = conn.transaction(|conn| {
            // collect product ids for the given crawler
            let ids: Vec<i32> = products::table
                .filter(products::crawler_id.eq(crawler_id))
                .select(products::id)
                .load(conn)?;

            if !ids.is_empty() {
                diesel::delete(
                    product_benchmark::table.filter(product_benchmark::product_id.eq_any(&ids)),
                )
                .execute(conn)?;
            }

            diesel::delete(products::table.filter(products::crawler_id.eq(crawler_id)))
                .execute(conn)
        })?;

        Ok(deleted)
    }
}
