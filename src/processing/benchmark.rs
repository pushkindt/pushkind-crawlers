use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_dantes::domain::benchmark::Benchmark;
use pushkind_dantes::domain::types::{BenchmarkId, ProductId, SimilarityDistance};

use crate::SIMILARITY_THRESHOLD;
use crate::processing::embedding::{
    load_or_generate_embedding, product_embedding_prompt, search_top_k,
};
use crate::repository::{
    BenchmarkReader, BenchmarkWriter, CrawlerReader, ProductReader, ProductWriter,
};

/// Generate embeddings for a benchmark and related products, build a search
/// index and update benchmark-product associations.
///
/// The function fetches the benchmark and all products for the same hub,
/// generates missing embeddings using the multilingual E5 model, persists
/// them, then builds a cosine index with `usearch` to find the closest
/// products. Associations in the database are replaced with the top results
/// and the benchmark processing flag is updated when complete.
pub async fn process_benchmark_message<R>(benchmark_id: BenchmarkId, repo: R)
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

    let benchmark_prompt = product_embedding_prompt(
        benchmark.name.as_str(),
        benchmark.sku.as_str(),
        benchmark.category.as_str(),
        benchmark.units.as_str(),
        benchmark.price.get(),
        benchmark.amount.get(),
        benchmark.description.as_str(),
    );
    let benchmark_embedding = match load_or_generate_embedding(
        benchmark.embedding.as_deref(),
        benchmark_prompt,
        &mut embedder,
        |embedding| {
            repo.set_benchmark_embedding(benchmark.id, embedding)
                .map(|_| ())
                .map_err(|error| format!("Failed to set benchmark embedding: {error:?}"))
        },
    ) {
        Ok((embedding, _generated)) => embedding,
        Err(error) => {
            log::error!(
                "Failed to resolve benchmark embedding for benchmark {}: {error}",
                benchmark.id
            );
            return;
        }
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
            let product_prompt = product_embedding_prompt(
                product.name.as_str(),
                product.sku.as_str(),
                product.category.as_deref().unwrap_or(""),
                product.units.as_deref().unwrap_or(""),
                product.price.get(),
                product.amount.map(|value| value.get()).unwrap_or_default(),
                product.description.as_deref().unwrap_or(""),
            );
            let embedding = match load_or_generate_embedding(
                product.embedding.as_deref(),
                product_prompt,
                &mut embedder,
                |value| {
                    repo.set_product_embedding(product.id, value)
                        .map(|_| ())
                        .map_err(|error| format!("Failed to set product embedding: {error:?}"))
                },
            ) {
                Ok((embedding, _generated)) => embedding,
                Err(error) => {
                    log::error!(
                        "Failed to resolve product embedding for product {}: {error}",
                        product.id
                    );
                    return;
                }
            };

            product_embeddings.push((product.id.get(), embedding));
        }

        let top_10_products = match search_top_k(&benchmark_embedding, &product_embeddings, 10) {
            Ok(top_10_products) => top_10_products,
            Err(e) => {
                log::error!("Failed to search top 10 products: {e:?}");
                return;
            }
        };

        for (key, distance) in top_10_products {
            let distance = 1.0 - distance;
            if distance < SIMILARITY_THRESHOLD {
                continue;
            }
            let product_id = match ProductId::new(key as i32) {
                Ok(product_id) => product_id,
                Err(e) => {
                    log::warn!("Skipping invalid product id from similarity index: {e}");
                    continue;
                }
            };
            let similarity_distance = match SimilarityDistance::new(distance) {
                Ok(similarity_distance) => similarity_distance,
                Err(e) => {
                    log::warn!("Skipping invalid similarity distance: {e}");
                    continue;
                }
            };
            if let Err(e) =
                repo.set_benchmark_association(benchmark_id, product_id, similarity_distance)
            {
                log::error!("Failed to set association: {e:?}");
                return;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_produces_expected_string() {
        let result = product_embedding_prompt(
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
