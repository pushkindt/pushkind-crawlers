use bytemuck::cast_slice;
use diesel::prelude::*;
use pushkind_common::repository::errors::{RepositoryError, RepositoryResult};
use pushkind_dantes::domain::category::Category;
use pushkind_dantes::domain::types::{
    CategoryAssignmentSource, CategoryId, CrawlerId, HubId, ProductId,
};
use pushkind_dantes::models::category::Category as DbCategory;

use crate::repository::{
    CategoryReader, CategoryWriter, DieselRepository, ProcessingGuardReader, ProcessingGuardWriter,
    ProductCategoryWriter,
};

impl CategoryReader for DieselRepository {
    fn list_categories(&self, hub_id: HubId) -> RepositoryResult<Vec<Category>> {
        use pushkind_dantes::schema::categories;

        let mut conn = self.conn()?;

        let result = categories::table
            .filter(categories::hub_id.eq(hub_id.get()))
            .load::<DbCategory>(&mut conn)?;

        result
            .into_iter()
            .map(Category::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| RepositoryError::ValidationError(err.to_string()))
    }
}

impl CategoryWriter for DieselRepository {
    fn set_category_embedding(
        &self,
        category_id: CategoryId,
        embedding: &[f32],
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::categories;

        let mut conn = self.conn()?;
        let blob: Vec<u8> = cast_slice(embedding).to_vec();

        let affected =
            diesel::update(categories::table.filter(categories::id.eq(category_id.get())))
                .set(categories::embedding.eq(blob))
                .execute(&mut conn)?;

        Ok(affected)
    }
}

impl ProductCategoryWriter for DieselRepository {
    fn set_product_category_automatic(
        &self,
        product_id: ProductId,
        category_id: Option<CategoryId>,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::products;

        let mut conn = self.conn()?;

        let affected = diesel::update(
            products::table
                .filter(products::id.eq(product_id.get()))
                .filter(
                    products::category_assignment_source
                        .ne(CategoryAssignmentSource::Manual.as_str()),
                ),
        )
        .set((
            products::category_id.eq(category_id.map(|value| value.get())),
            products::category_assignment_source.eq(CategoryAssignmentSource::Automatic.as_str()),
            products::updated_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)?;

        Ok(affected)
    }

    fn clear_product_categories_by_crawler(
        &self,
        crawler_id: CrawlerId,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::products;

        let mut conn = self.conn()?;

        let affected = diesel::update(
            products::table
                .filter(products::crawler_id.eq(crawler_id.get()))
                .filter(
                    products::category_assignment_source
                        .ne(CategoryAssignmentSource::Manual.as_str()),
                ),
        )
        .set((
            products::category_id.eq::<Option<i32>>(None),
            products::category_assignment_source.eq(CategoryAssignmentSource::Automatic.as_str()),
            products::updated_at.eq(diesel::dsl::now),
        ))
        .execute(&mut conn)?;

        Ok(affected)
    }
}

impl ProcessingGuardReader for DieselRepository {
    fn has_any_processing_in_hub(&self, hub_id: HubId) -> RepositoryResult<bool> {
        use pushkind_dantes::schema::{benchmarks, crawlers};

        let mut conn = self.conn()?;

        let active_crawlers = crawlers::table
            .filter(crawlers::hub_id.eq(hub_id.get()))
            .filter(crawlers::processing.eq(true))
            .count()
            .get_result::<i64>(&mut conn)?;

        if active_crawlers > 0 {
            return Ok(true);
        }

        let active_benchmarks = benchmarks::table
            .filter(benchmarks::hub_id.eq(hub_id.get()))
            .filter(benchmarks::processing.eq(true))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(active_benchmarks > 0)
    }
}

impl ProcessingGuardWriter for DieselRepository {
    fn set_hub_crawlers_processing(
        &self,
        hub_id: HubId,
        processing: bool,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::crawlers;

        let mut conn = self.conn()?;

        let affected = diesel::update(crawlers::table.filter(crawlers::hub_id.eq(hub_id.get())))
            .set(crawlers::processing.eq(processing))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn set_hub_benchmarks_processing(
        &self,
        hub_id: HubId,
        processing: bool,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::benchmarks;

        let mut conn = self.conn()?;

        let affected =
            diesel::update(benchmarks::table.filter(benchmarks::hub_id.eq(hub_id.get())))
                .set(benchmarks::processing.eq(processing))
                .execute(&mut conn)?;

        Ok(affected)
    }
}
