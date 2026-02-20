use std::error::Error;

use bytemuck::cast_slice;
use fastembed::TextEmbedding;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// Build a textual prompt describing a benchmark or product for embedding.
///
/// The prompt includes the following fields in order: name, SKU, category,
/// units, price, amount and description.
pub(crate) fn product_embedding_prompt(
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
pub(crate) fn normalize_embedding(vec: &[f32]) -> Vec<f32> {
    let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        vec.to_vec()
    } else {
        vec.iter().map(|x| x / norm).collect()
    }
}

/// Load an embedding from blob when present, otherwise generate and persist it.
///
/// Returns the embedding and a flag indicating whether a new embedding was
/// generated.
pub(crate) fn load_or_generate_embedding<F>(
    existing_blob: Option<&[u8]>,
    prompt: String,
    embedder: &mut TextEmbedding,
    persist: F,
) -> Result<(Vec<f32>, bool), String>
where
    F: FnOnce(&[f32]) -> Result<(), String>,
{
    if let Some(blob) = existing_blob {
        return Ok((cast_slice(blob).to_vec(), false));
    }

    let generated = embedder
        .embed(vec![prompt], None)
        .map_err(|error| format!("Failed to generate embedding: {error:?}"))?
        .into_iter()
        .next()
        .map(|value| normalize_embedding(&value))
        .unwrap_or_default();

    persist(&generated)?;

    Ok((generated, true))
}

/// Search the top-k closest vectors to the query embedding.
pub(crate) fn search_top_k<'a, T>(
    query_embedding: &[f32],
    items: &'a [(i32, T)],
    k: usize,
) -> Result<Vec<(u64, f32)>, Box<dyn Error>>
where
    T: AsRef<[f32]> + 'a,
{
    if items.is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    let dim = query_embedding.len();

    let index = Index::new(&IndexOptions {
        dimensions: dim,
        metric: MetricKind::Cos,
        quantization: ScalarKind::F32,
        ..Default::default()
    })?;

    index.reserve(items.len())?;

    for (id, embedding) in items {
        index.add(*id as u64, embedding.as_ref())?;
    }

    let neighbors = index.search(query_embedding, k)?;

    let results: Vec<(u64, f32)> = neighbors
        .keys
        .iter()
        .zip(neighbors.distances.iter())
        .map(|(&key, &distance)| (key, distance))
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::search_top_k;

    #[test]
    fn search_top_k_returns_empty_for_empty_items() {
        let query = vec![1.0_f32, 0.0, 0.0];
        let items: Vec<(i32, Vec<f32>)> = Vec::new();

        let result = search_top_k(&query, &items, 1).expect("search should succeed");

        assert!(result.is_empty());
    }

    #[test]
    fn search_top_k_returns_best_neighbor_first() {
        let query = vec![1.0_f32, 0.0, 0.0];
        let items = vec![
            (10, vec![0.0_f32, 1.0, 0.0]),
            (20, vec![1.0_f32, 0.0, 0.0]),
            (30, vec![0.5_f32, 0.5, 0.0]),
        ];

        let result = search_top_k(&query, &items, 1).expect("search should succeed");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 20);
    }
}
