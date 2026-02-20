use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use pushkind_dantes::domain::types::HubId;

use crate::SIMILARITY_THRESHOLD;
use crate::processing::embedding::{
    load_or_generate_embedding, product_embedding_prompt, search_top_k,
};
use crate::repository::{
    CategoryReader, CategoryWriter, CrawlerReader, ProcessingGuardReader, ProcessingGuardWriter,
    ProductCategoryWriter, ProductReader, ProductWriter,
};

/// Category prompt for category-directory embeddings.
///
/// The feature spec requires category name only.
fn category_prompt(name: &str) -> String {
    name.to_string()
}

#[derive(Default)]
struct MatchStats {
    categories_loaded: usize,
    products_loaded: usize,
    category_embeddings_generated: usize,
    product_embeddings_generated: usize,
    matched: usize,
    unmatched: usize,
    skipped_below_threshold: usize,
    skipped_invalid_category_id: usize,
    skipped_no_category_candidate: usize,
}

fn process_product_category_match<R>(hub_id: HubId, repo: &R) -> Result<MatchStats, ()>
where
    R: CrawlerReader
        + ProductReader
        + ProductWriter
        + CategoryReader
        + CategoryWriter
        + ProductCategoryWriter,
{
    let mut stats = MatchStats::default();

    let mut embedder =
        match TextEmbedding::try_new(InitOptions::new(EmbeddingModel::MultilingualE5Large)) {
            Ok(embedder) => embedder,
            Err(error) => {
                log::error!("Failed to initialize embedder for hub {hub_id}: {error:?}");
                return Err(());
            }
        };

    let crawlers = match repo.list_crawlers(hub_id) {
        Ok(crawlers) => crawlers,
        Err(error) => {
            log::error!("Failed to list crawlers for hub {hub_id}: {error:?}");
            return Err(());
        }
    };

    let mut products = Vec::new();
    for crawler in crawlers {
        let crawler_products = match repo.list_products(crawler.id) {
            Ok(products) => products,
            Err(error) => {
                log::error!(
                    "Failed to list products for crawler {} in hub {hub_id}: {error:?}",
                    crawler.id
                );
                return Err(());
            }
        };
        products.extend(crawler_products);
    }

    stats.products_loaded = products.len();

    let categories = match repo.list_categories(hub_id) {
        Ok(categories) => categories,
        Err(error) => {
            log::error!("Failed to list categories for hub {hub_id}: {error:?}");
            return Err(());
        }
    };
    stats.categories_loaded = categories.len();

    let mut category_embeddings: Vec<(i32, Vec<f32>)> = Vec::with_capacity(categories.len());
    for category in categories {
        let category_text = category_prompt(category.name.as_str());
        let embedding = match load_or_generate_embedding(
            category.embedding.as_deref(),
            category_text,
            &mut embedder,
            |value| {
                repo.set_category_embedding(category.id, value)
                    .map(|_| ())
                    .map_err(|error| {
                        format!(
                            "Failed to persist category embedding for {} in hub {hub_id}: {error:?}",
                            category.id
                        )
                    })
            },
        ) {
            Ok((embedding, generated)) => {
                if generated {
                    stats.category_embeddings_generated += 1;
                }
                embedding
            }
            Err(error) => {
                log::error!(
                    "Failed to resolve category embedding for {} in hub {hub_id}: {error}",
                    category.id
                );
                return Err(());
            }
        };

        category_embeddings.push((category.id.get(), embedding));
    }

    if stats.categories_loaded == 0 && stats.products_loaded > 0 {
        log::warn!(
            "No categories found for hub {hub_id}; all {} products will be set to NULL category_id",
            stats.products_loaded
        );
    }

    for product in products {
        let product_text = product_embedding_prompt(
            product.name.as_str(),
            product.sku.as_str(),
            product.category.as_deref().unwrap_or(""),
            product.units.as_deref().unwrap_or(""),
            product.price.get(),
            product.amount.map(|value| value.get()).unwrap_or_default(),
            product.description.as_deref().unwrap_or(""),
        );
        let product_embedding = match load_or_generate_embedding(
            product.embedding.as_deref(),
            product_text,
            &mut embedder,
            |value| {
                repo.set_product_embedding(product.id, value)
                    .map(|_| ())
                    .map_err(|error| {
                        format!(
                            "Failed to persist product embedding for {} in hub {hub_id}: {error:?}",
                            product.id
                        )
                    })
            },
        ) {
            Ok((embedding, generated)) => {
                if generated {
                    stats.product_embeddings_generated += 1;
                }
                embedding
            }
            Err(error) => {
                log::error!(
                    "Failed to resolve product embedding for {} in hub {hub_id}: {error}",
                    product.id
                );
                return Err(());
            }
        };

        let assigned_category = match search_top_k(&product_embedding, &category_embeddings, 1) {
            Ok(results) => match results.into_iter().next() {
                Some((key, distance)) => {
                    let similarity = 1.0 - distance;
                    if similarity < SIMILARITY_THRESHOLD {
                        stats.skipped_below_threshold += 1;
                        None
                    } else {
                        match i32::try_from(key)
                            .ok()
                            .and_then(|id| pushkind_dantes::domain::types::CategoryId::new(id).ok())
                        {
                            Some(category_id) => Some(category_id),
                            None => {
                                stats.skipped_invalid_category_id += 1;
                                log::warn!(
                                    "Skipping invalid category id {key} from similarity index for product {}",
                                    product.id
                                );
                                None
                            }
                        }
                    }
                }
                None => {
                    stats.skipped_no_category_candidate += 1;
                    None
                }
            },
            Err(error) => {
                log::error!(
                    "Failed to run top-1 category search for product {}: {error:?}",
                    product.id
                );
                return Err(());
            }
        };

        if let Err(error) = repo.set_product_category_automatic(product.id, assigned_category) {
            log::error!(
                "Failed to set product category assignment for product {} in hub {hub_id}: {error:?}",
                product.id
            );
            return Err(());
        }

        if assigned_category.is_some() {
            stats.matched += 1;
        } else {
            stats.unmatched += 1;
        }
    }

    Ok(stats)
}

fn run_with_hub_processing_guard<R, F, T>(hub_id: HubId, repo: &R, job: F) -> Result<Option<T>, ()>
where
    R: ProcessingGuardReader + ProcessingGuardWriter,
    F: FnOnce() -> Result<T, ()>,
{
    let already_processing = match repo.has_any_processing_in_hub(hub_id) {
        Ok(value) => value,
        Err(error) => {
            log::error!("Failed to check processing guard for hub {hub_id}: {error:?}");
            return Err(());
        }
    };

    if already_processing {
        log::warn!(
            "Skipping ProductCategoryMatch for hub {hub_id}: processing already active (skipped_because_processing_active=1)"
        );
        return Ok(None);
    }

    if let Err(error) = repo.set_hub_crawlers_processing(hub_id, true) {
        log::error!("Failed to set crawler processing guard for hub {hub_id}: {error:?}");
        return Err(());
    }

    if let Err(error) = repo.set_hub_benchmarks_processing(hub_id, true) {
        log::error!("Failed to set benchmark processing guard for hub {hub_id}: {error:?}");
        if let Err(reset_error) = repo.set_hub_crawlers_processing(hub_id, false) {
            log::error!(
                "Failed to rollback crawler processing guard for hub {hub_id}: {reset_error:?}"
            );
        }
        return Err(());
    }

    let outcome = job();

    if let Err(error) = repo.set_hub_crawlers_processing(hub_id, false) {
        log::error!("Failed to reset crawler processing guard for hub {hub_id}: {error:?}");
    }
    if let Err(error) = repo.set_hub_benchmarks_processing(hub_id, false) {
        log::error!("Failed to reset benchmark processing guard for hub {hub_id}: {error:?}");
    }

    match outcome {
        Ok(value) => Ok(Some(value)),
        Err(()) => Err(()),
    }
}

/// Handle product-to-category matching messages.
pub async fn process_product_category_match_message<R>(hub_id: HubId, repo: R)
where
    R: CrawlerReader
        + ProductReader
        + ProductWriter
        + CategoryReader
        + CategoryWriter
        + ProductCategoryWriter
        + ProcessingGuardReader
        + ProcessingGuardWriter,
{
    log::info!("Received ProductCategoryMatch for hub {hub_id}");

    let outcome = match run_with_hub_processing_guard(hub_id, &repo, || {
        process_product_category_match(hub_id, &repo)
    }) {
        Ok(Some(stats)) => Ok(stats),
        Ok(None) => return,
        Err(()) => Err(()),
    };

    match outcome {
        Ok(stats) => {
            log::info!(
                "Finished ProductCategoryMatch for hub {hub_id}: categories_loaded={}, products_loaded={}, category_embeddings_generated={}, product_embeddings_generated={}, matched={}, unmatched={}, skipped_below_threshold={}, skipped_invalid_category_id={}, skipped_no_category_candidate={}",
                stats.categories_loaded,
                stats.products_loaded,
                stats.category_embeddings_generated,
                stats.product_embeddings_generated,
                stats.matched,
                stats.unmatched,
                stats.skipped_below_threshold,
                stats.skipped_invalid_category_id,
                stats.skipped_no_category_candidate
            );
            if stats.skipped_below_threshold > 0
                || stats.skipped_invalid_category_id > 0
                || stats.skipped_no_category_candidate > 0
            {
                log::warn!(
                    "ProductCategoryMatch for hub {hub_id} had skipped assignments: below_threshold={}, invalid_category_id={}, no_candidate={}",
                    stats.skipped_below_threshold,
                    stats.skipped_invalid_category_id,
                    stats.skipped_no_category_candidate
                );
            }
        }
        Err(()) => {
            log::error!("ProductCategoryMatch failed for hub {hub_id}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pushkind_common::repository::errors::{RepositoryError, RepositoryResult};
    use pushkind_dantes::domain::types::HubId;

    use super::{category_prompt, run_with_hub_processing_guard};
    use crate::repository::{ProcessingGuardReader, ProcessingGuardWriter};

    #[derive(Default)]
    struct GuardState {
        has_any_processing: bool,
        fail_set_benchmarks_true: bool,
        crawlers_processing: bool,
        benchmarks_processing: bool,
        events: Vec<String>,
    }

    #[derive(Default)]
    struct FakeGuardRepo {
        state: Mutex<GuardState>,
    }

    impl FakeGuardRepo {
        fn with_state(has_any_processing: bool, fail_set_benchmarks_true: bool) -> Self {
            Self {
                state: Mutex::new(GuardState {
                    has_any_processing,
                    fail_set_benchmarks_true,
                    ..Default::default()
                }),
            }
        }

        fn mark(&self, event: &str) {
            let mut state = self.state.lock().expect("state mutex poisoned");
            state.events.push(event.to_string());
        }

        fn flags(&self) -> (bool, bool) {
            let state = self.state.lock().expect("state mutex poisoned");
            (state.crawlers_processing, state.benchmarks_processing)
        }

        fn events(&self) -> Vec<String> {
            let state = self.state.lock().expect("state mutex poisoned");
            state.events.clone()
        }
    }

    impl ProcessingGuardReader for FakeGuardRepo {
        fn has_any_processing_in_hub(&self, _hub_id: HubId) -> RepositoryResult<bool> {
            let state = self.state.lock().expect("state mutex poisoned");
            Ok(state.has_any_processing)
        }
    }

    impl ProcessingGuardWriter for FakeGuardRepo {
        fn set_hub_crawlers_processing(
            &self,
            _hub_id: HubId,
            processing: bool,
        ) -> RepositoryResult<usize> {
            let mut state = self.state.lock().expect("state mutex poisoned");
            state.crawlers_processing = processing;
            state
                .events
                .push(format!("set_hub_crawlers_processing({processing})"));
            Ok(1)
        }

        fn set_hub_benchmarks_processing(
            &self,
            _hub_id: HubId,
            processing: bool,
        ) -> RepositoryResult<usize> {
            let mut state = self.state.lock().expect("state mutex poisoned");
            if processing && state.fail_set_benchmarks_true {
                state
                    .events
                    .push("set_hub_benchmarks_processing(true)->err".to_string());
                return Err(RepositoryError::Unexpected(
                    "injected benchmark-guard failure".to_string(),
                ));
            }
            state.benchmarks_processing = processing;
            state
                .events
                .push(format!("set_hub_benchmarks_processing({processing})"));
            Ok(1)
        }
    }

    #[test]
    fn category_prompt_uses_category_name_only() {
        assert_eq!(category_prompt("Green Tea"), "Green Tea");
    }

    #[test]
    fn guard_skips_when_processing_is_already_active() {
        let repo = FakeGuardRepo::with_state(true, false);
        let hub_id = HubId::new(1).expect("valid hub id");

        let result = run_with_hub_processing_guard(hub_id, &repo, || Ok(()));

        assert!(matches!(result, Ok(None)));
        assert!(repo.events().is_empty());
        assert_eq!(repo.flags(), (false, false));
    }

    #[test]
    fn guard_sets_true_before_job_and_resets_false_after_success() {
        let repo = FakeGuardRepo::with_state(false, false);
        let hub_id = HubId::new(1).expect("valid hub id");

        let result = run_with_hub_processing_guard(hub_id, &repo, || {
            repo.mark("job_started");
            assert_eq!(repo.flags(), (true, true));
            Ok("ok")
        });

        assert!(matches!(result, Ok(Some("ok"))));
        assert_eq!(repo.flags(), (false, false));
        assert_eq!(
            repo.events(),
            vec![
                "set_hub_crawlers_processing(true)".to_string(),
                "set_hub_benchmarks_processing(true)".to_string(),
                "job_started".to_string(),
                "set_hub_crawlers_processing(false)".to_string(),
                "set_hub_benchmarks_processing(false)".to_string(),
            ]
        );
    }

    #[test]
    fn guard_resets_flags_after_failure() {
        let repo = FakeGuardRepo::with_state(false, false);
        let hub_id = HubId::new(1).expect("valid hub id");

        let result: Result<Option<()>, ()> = run_with_hub_processing_guard(hub_id, &repo, || {
            repo.mark("job_started");
            Err(())
        });

        assert!(matches!(result, Err(())));
        assert_eq!(repo.flags(), (false, false));
        assert_eq!(
            repo.events(),
            vec![
                "set_hub_crawlers_processing(true)".to_string(),
                "set_hub_benchmarks_processing(true)".to_string(),
                "job_started".to_string(),
                "set_hub_crawlers_processing(false)".to_string(),
                "set_hub_benchmarks_processing(false)".to_string(),
            ]
        );
    }

    #[test]
    fn guard_rolls_back_crawlers_when_setting_benchmarks_true_fails() {
        let repo = FakeGuardRepo::with_state(false, true);
        let hub_id = HubId::new(1).expect("valid hub id");

        let result = run_with_hub_processing_guard(hub_id, &repo, || Ok(()));

        assert!(matches!(result, Err(())));
        assert_eq!(repo.flags(), (false, false));
        assert_eq!(
            repo.events(),
            vec![
                "set_hub_crawlers_processing(true)".to_string(),
                "set_hub_benchmarks_processing(true)->err".to_string(),
                "set_hub_crawlers_processing(false)".to_string(),
            ]
        );
    }
}
