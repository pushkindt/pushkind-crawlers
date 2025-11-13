use bytemuck::cast_slice;
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::QueryResult;
use pushkind_common::db::DbConnection;
use pushkind_common::domain::dantes::product::{NewProduct, Product};
use pushkind_common::models::dantes::product::{NewProduct as DbNewProduct, Product as DbProduct};
use pushkind_common::models::dantes::product_image::{NewProductImage, ProductImage};
use pushkind_common::repository::errors::{RepositoryError, RepositoryResult};
use std::collections::HashMap;

use crate::repository::DieselRepository;
use crate::repository::ProductReader;
use crate::repository::ProductWriter;

fn replace_product_images(
    conn: &mut DbConnection,
    product_id: i32,
    image_urls: &[String],
) -> QueryResult<()> {
    use pushkind_common::schema::dantes::product_images;

    diesel::delete(product_images::table.filter(product_images::product_id.eq(product_id)))
        .execute(conn)?;

    if image_urls.is_empty() {
        return Ok(());
    }

    let new_images = image_urls
        .iter()
        .map(|url| NewProductImage {
            product_id,
            url: url.clone(),
        })
        .collect::<Vec<_>>();

    diesel::insert_into(product_images::table)
        .values(&new_images)
        .execute(conn)?;

    Ok(())
}

impl ProductReader for DieselRepository {
    fn list_products(&self, crawler_id: i32) -> RepositoryResult<Vec<Product>> {
        use pushkind_common::schema::dantes::{product_images, products};

        let mut conn = self.conn()?;

        let products: Vec<DbProduct> = products::table
            .filter(products::crawler_id.eq(crawler_id))
            .load::<DbProduct>(&mut conn)?;

        let product_ids: Vec<i32> = products.iter().map(|p| p.id).collect();
        let mut images_by_product = HashMap::new();
        if !product_ids.is_empty() {
            let images = product_images::table
                .filter(product_images::product_id.eq_any(&product_ids))
                .load::<ProductImage>(&mut conn)?;
            for image in images {
                images_by_product
                    .entry(image.product_id)
                    .or_insert_with(Vec::new)
                    .push(image.url);
            }
        }

        Ok(products
            .into_iter()
            .map(|db_product| {
                let image_urls = images_by_product.remove(&db_product.id).unwrap_or_default();
                let mut product: Product = db_product.into();
                product.images = image_urls;
                product
            })
            .collect())
    }
}

impl ProductWriter for DieselRepository {
    fn create_products(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        if products.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn()?;
        let inserted = conn.transaction(|conn| {
            let mut inserted_rows = 0;
            for product in products.iter() {
                let db_product: DbNewProduct = product.clone().into();
                let product_id = diesel::insert_into(products::table)
                    .values(&db_product)
                    .returning(products::id)
                    .get_result::<i32>(conn)?;
                replace_product_images(conn, product_id, &product.images)?;
                inserted_rows += 1;
            }
            Ok::<usize, RepositoryError>(inserted_rows)
        })?;

        Ok(inserted)
    }

    fn update_products(&self, products: &[NewProduct]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::products;

        let mut conn = self.conn()?;

        if products.is_empty() {
            return Ok(0);
        }

        let affected = conn.transaction(|conn| {
            let mut affected_rows = 0;
            for product in products.iter() {
                let db_product: DbNewProduct = product.clone().into();
                let product_id = diesel::insert_into(products::table)
                    .values(&db_product)
                    .on_conflict((products::crawler_id, products::url))
                    .do_update()
                    .set((&db_product, products::updated_at.eq(Utc::now().naive_utc())))
                    .returning(products::id)
                    .get_result::<i32>(conn)?;
                replace_product_images(conn, product_id, &product.images)?;
                affected_rows += 1;
            }
            Ok::<usize, RepositoryError>(affected_rows)
        })?;

        Ok(affected)
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
