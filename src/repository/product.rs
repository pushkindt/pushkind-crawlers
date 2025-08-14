use bytemuck::cast_slice;
use chrono::Utc;
use diesel::prelude::*;
use pushkind_common::domain::product::{NewProduct, Product};
use pushkind_common::models::product::{NewProduct as DbNewProduct, Product as DbProduct};
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::DieselRepository;
use crate::repository::ProductReader;
use crate::repository::ProductWriter;

impl ProductReader for DieselRepository {
    fn list_products(&self, crawler_id: i32) -> RepositoryResult<Vec<Product>> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

        let products: Vec<DbProduct> = products::table
            .filter(products::crawler_id.eq(crawler_id))
            .load::<DbProduct>(&mut conn)?;

        Ok(products.into_iter().map(Into::into).collect())
    }
}

impl ProductWriter for DieselRepository {
    fn create_products(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

        // Convert domain objects into their database representation
        let new_products: Vec<DbNewProduct> = products.iter().cloned().map(Into::into).collect();

        let inserted = diesel::insert_into(products::table)
            .values(&new_products)
            .execute(&mut conn)?;

        Ok(inserted)
    }

    fn update_products(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

        let mut affected_rows = 0;
        for product in products.iter().cloned() {
            let db_product: DbNewProduct = product.into();
            // Upsert by crawler and url, touching updated_at when a row exists
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

    fn set_product_embedding(&self, product_id: i32, embedding: &[f32]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

        // Convert &[f32] to &[u8]
        let blob: Vec<u8> = cast_slice(embedding).to_vec();

        let affected = diesel::update(products::table.filter(products::id.eq(product_id)))
            .set(products::embedding.eq(blob))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn delete_products(&self, crawler_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::{product_benchmark, products};

        let mut conn = self.conn()?;

        let deleted = conn.transaction(|conn| {
            // Fetch product ids to cascade delete related benchmark associations
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
