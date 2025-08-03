use bytemuck::cast_slice;
use diesel::prelude::*;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_common::db::DbPool;
use pushkind_common::models::benchmark::Benchmark as DbBenchmark;
use pushkind_common::models::product::Product as DbProduct;
use pushkind_common::schema::dantes::{benchmarks, products};
// use usearch::index::{Index, IndexOptions, MetricKind};

use crate::repository::benchmark::DieselBenchmarkRepository;
use crate::repository::product::DieselProductRepository;
use crate::repository::{BenchmarkReader, BenchmarkWriter, ProductReader, ProductWriter};

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

pub async fn process_benchmark_message(benchmark_id: i32, db_pool: &DbPool) {
    log::info!("Received benchmark: {benchmark_id:?}");

    let product_repo = DieselProductRepository::new(db_pool);
    let benchmark_repo = DieselBenchmarkRepository::new(db_pool);

    let benchmark = match benchmark_repo.get(benchmark_id) {
        Ok(benchmark) => benchmark,
        Err(e) => {
            log::error!("Failed to fetch benchmark: {e:?}");
            return;
        }
    };

    // Initialize embedder for multilingual E5 large
    let mut embedder =
        match TextEmbedding::try_new(InitOptions::new(EmbeddingModel::MultilingualE5Large)) {
            Ok(embedder) => embedder,
            Err(e) => {
                log::error!("Failed to initialize embedder: {e:?}");
                return;
            }
        };

    // Fetch all products

    let products = match product_repo.list(benchmark.hub_id) {
        Ok(products) => products,
        Err(e) => {
            log::error!("Failed to fetch products: {e:?}");
            return;
        }
    };

    // // Collect embeddings for index
    // let mut product_embeddings: Vec<(i32, Vec<f32>)> = Vec::new();

    // for product in db_products {
    //     let embedding: Vec<f32> = if let Some(blob) = product.embedding.clone() {
    //         cast_slice(&blob).to_vec()
    //     } else {
    //         let text = prompt(
    //             &product.name,
    //             &product.sku,
    //             product.category.as_deref().unwrap_or(""),
    //             product.units.as_deref().unwrap_or(""),
    //             product.price,
    //             product.amount.unwrap_or_default(),
    //             product.description.as_deref().unwrap_or(""),
    //         );

    //         let emb = embedder.embed(vec![text], None).expect("embed product");
    //         let emb = emb.first().cloned().unwrap_or_default();
    //         product_repo
    //             .set_embedding(product.id, &emb)
    //             .expect("set product embedding");
    //         emb
    //     };

    //     product_embeddings.push((product.id, embedding));
    // }

    // let benchmark_embedding: Vec<f32> = if let Some(blob) = bench.embedding.clone() {
    //     cast_slice(&blob).to_vec()
    // } else {
    //     let text = prompt(
    //         &bench.name,
    //         &bench.sku,
    //         &bench.category,
    //         &bench.units,
    //         bench.price,
    //         bench.amount,
    //         &bench.description,
    //     );

    //     let emb = embedder
    //         .embed(vec![text], None)
    //         .expect("embed benchmark")
    //         .into_iter()
    //         .next()
    //         .unwrap_or_default();
    //     benchmark_repo
    //         .set_embedding(bench.id, &emb)
    //         .expect("set benchmark embedding");
    //     emb
    // };

    // // Build search index
    // let dim = benchmark_embedding.len();
    // let mut index = match Index::new(IndexOptions {
    //     dimensions: dim,
    //     metric: MetricKind::Cos,
    //     ..Default::default()
    // }){
    //     Ok(index) => index,
    //     Err(e) => {
    //         log::error!("Failed to create index: {e:?}");
    //         return;
    //     }
    // };

    // for (id, emb) in &product_embeddings {
    //     match index.add(*id as u64, emb) {
    //         Ok(_) => (),
    //         Err(e) => {
    //             log::error!("Failed to add product to index: {e:?}");
    //             return;
    //         }
    //     }
    // }

    // // Search for top 10 closest products
    // let neighbors = match index.search(&benchmark_embedding, 10) {
    //     Ok(neighbors) => neighbors,
    //     Err(e) => {
    //         log::error!("Failed to search index: {e:?}");
    //         return;
    //     }
    // };

    // // Update associations
    // if let Err(e) = benchmark_repo.clear_associations(benchmark_id) {
    //     log::error!("Failed to clear associations: {e:?}");
    //     return;
    // }

    // for neighbor in neighbors {
    //     let product_id = neighbor.key as i32;
    //     if let Err(e) = benchmark_repo.set_association(benchmark_id, product_id) {
    //         log::error!("Failed to set association: {e:?}");
    //         return;
    //     }
    // }

    log::info!("Finished processing benchmar: {benchmark_id}");
}
