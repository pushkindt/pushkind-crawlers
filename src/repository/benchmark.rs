use bytemuck::cast_slice;
use chrono::Utc;
use diesel::prelude::*;
use pushkind_common::db::DbPool;
use pushkind_common::domain::benchmark::{NewBenchmark, Benchmark};
use pushkind_common::models::benchmark::{NewBenchmark as DbNewBenchmark, Benchmark as DbBenchmark};
use pushkind_common::repository::errors::RepositoryResult;

use crate::repository::BenchmarkReader;
use crate::repository::BenchmarkWriter;

pub struct DieselBenchmarkRepository<'a> {
    pub pool: &'a DbPool,
}

impl<'a> DieselBenchmarkRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }
}

impl BenchmarkReader for DieselBenchmarkRepository<'_> {
    fn get(&self, benchmark_id: i32) -> RepositoryResult<Benchmark> {
        use pushkind_common::schema::dantes::benchmarks;

        let mut conn = self.pool.get()?;

        let benchmark: DbBenchmark = benchmarks::table
            .filter(benchmarks::benchmark_id.eq(benchmark_id))
            .first(&mut conn)?;

        Ok(benchmark.into())
    }
}

impl BenchmarkWriter for DieselBenchmarkRepository<'_> {
    fn set_embedding(&self, benchmark_id: i32, embedding: &[f32]) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::benchmarks;

        let mut conn = self.pool.get()?;

        // Convert &[f32] to &[u8]
        let blob: Vec<u8> = cast_slice(embedding).to_vec();

        let affected = diesel::update(benchmarks::table.filter(benchmarks::id.eq(benchmark_id)))
            .set(benchmarks::embedding.eq(blob))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn remove_associations(&self, benchmark_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::product_benchmark;

        let mut conn = self.pool.get()?;

        let affected = diesel::delete(product_benchmark::table.filter(product_benchmark::benchmark_id.eq(benchmark_id)))
            .execute(&mut conn)?;

        Ok(affected)
    }

    fn set_association(&self, benchmark_id: i32, product_id: i32) -> RepositoryResult<usize> {
        use pushkind_common::schema::dantes::product_benchmark;

        let mut conn = self.pool.get()?;

        let affected = diesel::insert_into(product_benchmark::table)
            .values((product_benchmark::benchmark_id.eq(benchmark_id), product_benchmark::product_id.eq(product_id)))
            .execute(&mut conn)?;

        Ok(affected)
    }
}
