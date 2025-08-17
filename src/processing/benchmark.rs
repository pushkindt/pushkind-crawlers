use std::error::Error;

use bytemuck::cast_slice;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_common::domain::benchmark::Benchmark;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use crate::repository::{
    BenchmarkReader, BenchmarkWriter, CrawlerReader, ProductReader, ProductWriter,
};

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

/// Normalize a vector to unit length.
///
/// Returns the original vector when the norm is zero.
fn normalize(vec: &[f32]) -> Vec<f32> {
    let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        vec.to_vec() // Clone into a Vec<f32>
    } else {
        vec.iter().map(|x| x / norm).collect()
    }
}

/// Generate embeddings for a benchmark and related products, build a search
/// index and update benchmark-product associations.
///
/// The function fetches the benchmark and all products for the same hub,
/// generates missing embeddings using the multilingual E5 model, persists
/// them, then builds a cosine index with `usearch` to find the closest
/// products. Associations in the database are replaced with the top results
/// and the benchmark processing flag is updated when complete.
pub async fn process_benchmark_message<R>(benchmark_id: i32, repo: R)
where
    R: BenchmarkReader + BenchmarkWriter + ProductReader + ProductWriter + CrawlerReader,
{
    log::info!("Received benchmark: {benchmark_id:?}");

    let benchmark = match repo.get_benchmark(benchmark_id) {
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

    if let Err(e) = repo.set_benchmark_processing(benchmark_id, true) {
        log::error!("Failed to set benchmark processing: {e:?}");
        return;
    }

    process_benchmark(benchmark, &repo);

    if let Err(e) = repo.update_benchmark_stats(benchmark_id) {
        log::error!("Failed to update benchmark stats: {e:?}");
    }

    log::info!("Finished processing benchmark: {benchmark_id}");
}
/// Core logic for processing a benchmark and updating associations.
fn process_benchmark<R>(benchmark: Benchmark, repo: &R)
where
    R: BenchmarkReader + BenchmarkWriter + ProductReader + ProductWriter + CrawlerReader,
{
    let benchmark_id = benchmark.id;
    // Initialize embedder for multilingual E5 large
    let mut embedder =
        match TextEmbedding::try_new(InitOptions::new(EmbeddingModel::MultilingualE5Large)) {
            Ok(embedder) => embedder,
            Err(e) => {
                log::error!("Failed to initialize embedder: {e:?}");
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
            Ok(emb) => normalize(&emb.into_iter().next().unwrap_or_default()),
            Err(e) => {
                log::error!("Failed to embed benchmark: {e:?}");
                return;
            }
        };
        if let Err(e) = repo.set_benchmark_embedding(benchmark.id, &emb) {
            log::error!("Failed to set benchmark embedding: {e:?}");
            return;
        }
        emb
    };

    let crawlers = match repo.list_crawlers(benchmark.hub_id) {
        Ok(crawlers) => crawlers,
        Err(e) => {
            log::error!("Failed to fetch crawlers: {e:?}");
            return;
        }
    };

    // Remove existing associations
    if let Err(e) = repo.remove_benchmark_associations(benchmark_id) {
        log::error!("Failed to clear associations: {e:?}");
        return;
    }

    for crawler in crawlers {
        log::info!("Processing products for crawler: {}", crawler.name);
        let products = match repo.list_products(crawler.id) {
            Ok(products) => products,
            Err(e) => {
                log::error!("Failed to fetch products: {e:?}");
                return;
            }
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
                    Ok(emb) => normalize(&emb.into_iter().next().unwrap_or_default()),
                    Err(e) => {
                        log::error!("Failed to embed product: {e:?}");
                        return;
                    }
                };
                if let Err(e) = repo.set_product_embedding(product.id, &emb) {
                    log::error!("Failed to set product embedding: {e:?}");
                    return;
                }
                emb
            };

            product_embeddings.push((product.id, embedding));
        }

        let top_10_products = match search_top_10(&benchmark_embedding, &product_embeddings) {
            Ok(top_10_products) => top_10_products,
            Err(e) => {
                log::error!("Failed to search top 10 products: {e:?}");
                return;
            }
        };

        let threshold = 0.8;
        for (key, distance) in top_10_products {
            let distance = 1.0 - distance;
            if distance < threshold {
                continue;
            }
            let product_id = key as i32;
            if let Err(e) = repo.set_benchmark_association(benchmark_id, product_id, distance) {
                log::error!("Failed to set association: {e:?}");
                return;
            }
        }
    }
}
/// Search the top 10 closest products to the given benchmark embedding.
fn search_top_10<'a, T>(
    benchmark_embedding: &[f32],
    products: &'a [(i32, T)],
) -> Result<Vec<(u64, f32)>, Box<dyn Error>>
where
    T: AsRef<[f32]> + 'a,
{
    let dim = benchmark_embedding.len();

    let index = Index::new(&IndexOptions {
        dimensions: dim,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    })?;

    index.reserve(products.len())?;

    for (id, emb) in products {
        index.add(*id as u64, emb.as_ref())?;
    }

    let neighbors = index.search(benchmark_embedding, 10)?;

    let results: Vec<(u64, f32)> = neighbors
        .keys
        .iter()
        .zip(neighbors.distances.iter())
        .map(|(&k, &d)| (k, d))
        .collect();

    Ok(results)
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
