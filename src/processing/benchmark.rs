use bytemuck::cast_slice;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_common::db::DbPool;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use crate::repository::benchmark::DieselBenchmarkRepository;
use crate::repository::product::DieselProductRepository;
use crate::repository::{BenchmarkReader, BenchmarkWriter, ProductReader, ProductWriter};

/// Build a textual prompt describing a benchmark or product for embedding.
///
/// The prompt includes the following fields in order: name, SKU, category,
/// units, price, amount and description.
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

/// Generate embeddings for a benchmark and related products, build a search
/// index and update benchmark-product associations.
///
/// The function fetches the benchmark and all products for the same hub,
/// generates missing embeddings using the multilingual E5 model, persists
/// them, then builds a cosine index with `usearch` to find the closest
/// products. Associations in the database are replaced with the top results
/// and the benchmark processing flag is updated when complete.
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

    if benchmark.processing {
        log::warn!("Benchmark {benchmark_id} is already running");
        return;
    }

    if let Err(e) = benchmark_repo.set_processing(benchmark_id, true) {
        log::error!("Failed to set benchmark processing: {e:?}");
        return;
    }

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

    let benchmark_embedding: Vec<f32> = if let Some(blob) = benchmark.embedding {
        cast_slice(&blob).to_vec()
    } else {
        let text = prompt(
            &benchmark.name,
            &benchmark.sku,
            &benchmark.category,
            &benchmark.units,
            benchmark.price,
            benchmark.amount,
            &benchmark.description,
        );

        let emb = match embedder.embed(vec![text], None) {
            Ok(emb) => emb.into_iter().next().unwrap_or_default(),
            Err(e) => {
                log::error!("Failed to embed benchmark: {e:?}");
                return;
            }
        };
        if let Err(e) = benchmark_repo.set_embedding(benchmark.id, &emb) {
            log::error!("Failed to set benchmark embedding: {e:?}");
            return;
        }
        emb
    };

    // Collect embeddings for index
    let mut product_embeddings: Vec<(i32, Vec<f32>)> = Vec::new();

    for product in products {
        let embedding: Vec<f32> = if let Some(blob) = product.embedding {
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

            let emb = match embedder.embed(vec![text], None) {
                Ok(emb) => emb.into_iter().next().unwrap_or_default(),
                Err(e) => {
                    log::error!("Failed to embed product: {e:?}");
                    return;
                }
            };
            if let Err(e) = product_repo.set_embedding(product.id, &emb) {
                log::error!("Failed to set product embedding: {e:?}");
                return;
            }
            emb
        };

        product_embeddings.push((product.id, embedding));
    }

    // Build search index
    let dim = benchmark_embedding.len();
    let index = match Index::new(&IndexOptions {
        dimensions: dim,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    }) {
        Ok(index) => index,
        Err(e) => {
            log::error!("Failed to create index: {e:?}");
            return;
        }
    };

    match index.reserve(product_embeddings.len()) {
        Ok(_) => (),
        Err(e) => {
            log::error!("Failed to reserve index: {e:?}");
            return;
        }
    }

    for (id, emb) in &product_embeddings {
        let id: u64 = *id as u64;
        match index.add(id, emb) {
            Ok(_) => (),
            Err(e) => {
                log::error!("Failed to add product to index: {e:?}");
                return;
            }
        }
    }

    // Search for top 10 closest products
    let neighbors = match index.search(&benchmark_embedding, 10) {
        Ok(neighbors) => neighbors,
        Err(e) => {
            log::error!("Failed to search index: {e:?}");
            return;
        }
    };

    // Update associations
    if let Err(e) = benchmark_repo.remove_associations(benchmark_id) {
        log::error!("Failed to clear associations: {e:?}");
        return;
    }

    let threshold = 0.84;
    for (key, distance) in neighbors.keys.iter().zip(neighbors.distances.iter()) {
        if *distance < threshold {
            continue;
        }
        let product_id = *key as i32;
        if let Err(e) = benchmark_repo.set_association(benchmark_id, product_id, *distance) {
            log::error!("Failed to set association: {e:?}");
            return;
        }
    }

    if let Err(e) = benchmark_repo.set_processing(benchmark_id, false) {
        log::error!("Failed to set benchmark processing: {e:?}");
    }

    log::info!("Finished processing benchmark: {benchmark_id}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_produces_expected_string() {
        let result = prompt(
            "Sample Name",
            "SKU123",
            "Category",
            "units",
            9.99,
            2.0,
            "Description",
        );

        let expected = "Name: Sample Name\nSKU: SKU123\nCategory: Category\nUnits: units\nPrice: 9.99\nAmount: 2\nDescription: Description";
        assert_eq!(result, expected);
    }
}
