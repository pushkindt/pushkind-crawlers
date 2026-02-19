use bytemuck::cast_slice;
use diesel::prelude::*;
use pushkind_common::repository::errors::{RepositoryError, RepositoryResult};
use pushkind_dantes::domain::benchmark::Benchmark;
use pushkind_dantes::domain::types::{BenchmarkId, ProductId, SimilarityDistance};
use pushkind_dantes::models::benchmark::Benchmark as DbBenchmark;

use crate::repository::BenchmarkReader;
use crate::repository::BenchmarkWriter;
use crate::repository::DieselRepository;

impl BenchmarkReader for DieselRepository {
    fn get_benchmark(&self, benchmark_id: BenchmarkId) -> RepositoryResult<Benchmark> {
        use pushkind_dantes::schema::benchmarks;

        let mut conn = self.conn()?;

        // Fetch a single benchmark by its primary key
        let benchmark: DbBenchmark = benchmarks::table
            .filter(benchmarks::id.eq(benchmark_id.get()))
            .first(&mut conn)?;

        Benchmark::try_from(benchmark)
            .map_err(|err| RepositoryError::ValidationError(err.to_string()))
    }
}

impl BenchmarkWriter for DieselRepository {
    fn set_benchmark_embedding(
        &self,
        benchmark_id: BenchmarkId,
        embedding: &[f32],
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::benchmarks;

        let mut conn = self.conn()?;

        // Convert &[f32] to &[u8]
        let blob: Vec<u8> = cast_slice(embedding).to_vec();

        let affected =
            diesel::update(benchmarks::table.filter(benchmarks::id.eq(benchmark_id.get())))
                .set(benchmarks::embedding.eq(blob))
                .execute(&mut conn)?;

        Ok(affected)
    }

    fn remove_benchmark_associations(&self, benchmark_id: BenchmarkId) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::product_benchmark;

        let mut conn = self.conn()?;

        // Delete all product links for this benchmark
        let affected = diesel::delete(
            product_benchmark::table.filter(product_benchmark::benchmark_id.eq(benchmark_id.get())),
        )
        .execute(&mut conn)?;

        Ok(affected)
    }

    fn set_benchmark_association(
        &self,
        benchmark_id: BenchmarkId,
        product_id: ProductId,
        distance: SimilarityDistance,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::product_benchmark;

        let mut conn = self.conn()?;

        // Insert association entry with similarity distance
        let affected = diesel::insert_into(product_benchmark::table)
            .values((
                product_benchmark::benchmark_id.eq(benchmark_id.get()),
                product_benchmark::product_id.eq(product_id.get()),
                product_benchmark::distance.eq(distance.get()),
            ))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn set_benchmark_processing(
        &self,
        benchmark_id: BenchmarkId,
        processing: bool,
    ) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::benchmarks;

        let mut conn = self.conn()?;

        let affected =
            diesel::update(benchmarks::table.filter(benchmarks::id.eq(benchmark_id.get())))
                .set(benchmarks::processing.eq(processing))
                .execute(&mut conn)?;

        Ok(affected)
    }

    fn update_benchmark_stats(&self, benchmark_id: BenchmarkId) -> RepositoryResult<usize> {
        use pushkind_dantes::schema::benchmarks;
        use pushkind_dantes::schema::product_benchmark;

        let mut conn = self.conn()?;

        // Count products for the benchmark to update statistics
        let product_count: i64 = product_benchmark::table
            .filter(product_benchmark::benchmark_id.eq(benchmark_id.get()))
            .count()
            .get_result(&mut conn)?;

        // Update timestamp, processing state and product count
        let result =
            diesel::update(benchmarks::table.filter(benchmarks::id.eq(benchmark_id.get())))
                .set((
                    benchmarks::updated_at.eq(diesel::dsl::now),
                    benchmarks::processing.eq(false),
                    benchmarks::num_products.eq(product_count as i32), // cast if needed
                ))
                .execute(&mut conn)?;

        Ok(result)
    }
}
