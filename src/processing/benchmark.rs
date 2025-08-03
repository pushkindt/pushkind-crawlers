use bytemuck::cast_slice;
use diesel::prelude::*;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_common::db::DbPool;
use pushkind_common::models::benchmark::Benchmark as DbBenchmark;
use pushkind_common::models::product::Product as DbProduct;
use pushkind_common::schema::dantes::{benchmarks, products};
use usearch::index::{Index, IndexOptions, MetricKind};

use crate::repository::benchmark::DieselBenchmarkRepository;
use crate::repository::product::DieselProductRepository;

fn prompt(
    name: &str,
    sku: &str,
    category: &str,
    units: &str,
    price: f64,
    amount: f64,
    description: &str,
) -> String {
    format!(
        "Name: {name}\nSKU: {sku}\nCategory: {category}\nUnits: {units}\nPrice: {price}\nAmount: {amount}\nDescription: {description}",
    )
}

pub async fn process_benchmark_message(msg: i32, db_pool: &DbPool) {
    log::info!("Received benchmark: {msg:?}");

    let product_repo = DieselProductRepository::new(db_pool);
    let benchmark_repo = DieselBenchmarkRepository::new(db_pool);

    // Initialize embedder for multilingual E5 large
    let mut embedder = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::MultilingualE5Large))
        .expect("failed to init embedder");

    // Fetch all products
    let mut conn = db_pool.get().expect("db connection");
    let db_products: Vec<DbProduct> = products::table.load(&mut conn).expect("load products");

    // Collect embeddings for index
    let mut product_embeddings: Vec<(i32, Vec<f32>)> = Vec::new();

    for product in db_products {
        let embedding: Vec<f32> = if let Some(blob) = product.embedding.clone() {
            cast_slice(&blob).to_vec()
        } else {
            let text = prompt(
                &product.name,
                &product.sku,
                product.category.as_deref().unwrap_or(""),
                product.units.as_deref().unwrap_or(""),
                product.price,
                product.amount.unwrap_or_default(),
                product.description.as_deref().unwrap_or(""),
            );

            let emb = embedder.embed(vec![text], None).expect("embed product");
            let emb = emb.first().cloned().unwrap_or_default();
            product_repo
                .set_embedding(product.id, &emb)
                .expect("set product embedding");
            emb
        };

        product_embeddings.push((product.id, embedding));
    }

    // Fetch benchmark
    let bench: DbBenchmark = benchmarks::table
        .filter(benchmarks::id.eq(msg))
        .first(&mut conn)
        .expect("benchmark not found");

    let benchmark_embedding: Vec<f32> = if let Some(blob) = bench.embedding.clone() {
        cast_slice(&blob).to_vec()
    } else {
        let text = prompt(
            &bench.name,
            &bench.sku,
            &bench.category,
            &bench.units,
            bench.price,
            bench.amount,
            &bench.description,
        );

        let emb = embedder
            .embed(vec![text], None)
            .expect("embed benchmark")
            .into_iter()
            .next()
            .unwrap_or_default();
        benchmark_repo
            .set_embedding(bench.id, &emb)
            .expect("set benchmark embedding");
        emb
    };

    // Build search index
    let dim = benchmark_embedding.len();
    let mut index = Index::new(IndexOptions {
        dimensions: dim,
        metric: MetricKind::Cos,
        ..Default::default()
    })
    .expect("create index");

    for (id, emb) in &product_embeddings {
        index.add(*id as u64, emb).expect("add to index");
    }

    // Search for top 10 closest products
    let neighbors = index
        .search(&benchmark_embedding, 10)
        .expect("search index");

    // Update associations
    benchmark_repo
        .remove_associations(msg)
        .expect("remove associations");

    for neighbor in neighbors {
        let product_id = neighbor.key as i32;
        benchmark_repo
            .set_association(msg, product_id)
            .expect("set association");
    }

    log::info!("Finished processing benchmar: {msg}");
}
