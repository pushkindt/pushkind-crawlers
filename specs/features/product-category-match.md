# Feature Spec: Product Category Match

Status: stable  
Created: 2026-02-20
Related: `SPEC.md`, `plans/product-category-match.md`, `specs/decisions/0001-product-category-match.md`

## 1. Summary

Add support for `ZMQCrawlerMessage::ProductCategoryMatch(hub_id)` to assign one
most relevant category-directory record to each product in the hub.

The category directory in `pushkind-dantes` is the source of truth for
categories. Products should store an optional category-directory foreign key
(`category_id`), populated by this matching job.

## 2. Goals

- Process a new ZeroMQ message variant for category matching.
- Match every product in the target hub against the category directory and pick
  a single best category candidate.
- Ensure category embeddings are generated and persisted when missing before
  similarity search.
- Reuse the existing embedding/search approach used for benchmark matching
  (cosine similarity on normalized vectors).
- Coordinate with crawler and benchmark `processing` flags to prevent overlap
  with other running jobs.

## 3. Non-Goals

- No crawler-side category extraction redesign.
- No schema ownership changes in this repository (schema remains owned by
  `pushkind-dantes`).
- No UI/API changes.

## 4. Inputs and Outputs

Input message:
- `ZMQCrawlerMessage::ProductCategoryMatch(hub_id)`

Input data scope:
- All crawlers belonging to `hub_id`
- All products belonging to those crawlers
- All category-directory records belonging to `hub_id`

Output effects:
- Product `category_id` is updated to the best-matching category ID, or cleared
  to `NULL` when no match candidate can be selected.
- Only `category_id` is persisted as the matching result; no similarity score
  is stored.
- Product `category_assignment_source` is set to `"automatic"` for rows updated
  by this job.
- Missing category embeddings are generated and persisted.
- Missing product embeddings are generated and persisted (same as benchmark flow).

## 5. Matching Rules

Core rule:
- Exactly one best category candidate is selected per product using cosine
  similarity.

Embedding behavior:
- If product embedding is present, reuse it.
- If product embedding is missing, generate from product prompt and persist.
- If category embedding is present, reuse it.
- If category embedding is missing, generate from category prompt and persist.

Similarity behavior:
- Build an ANN index over category embeddings for the hub.
- Query top-1 nearest category for each product.
- Convert distance to similarity using `similarity = 1.0 - distance`.
- Apply the shared crate-level threshold
  `pushkind_crawlers::SIMILARITY_THRESHOLD` (currently `0.8`), identical to
  benchmark matching.

Processing-flag guard:
- When a `ProductCategoryMatch` message is received, the job must not start if
  any crawler or benchmark row in the target `hub_id` already has
  `processing = true`.
- Before matching starts, set `processing = true` for all crawlers and all
  benchmarks in the target `hub_id`.
- After matching finishes, set `processing = false` for all crawlers and all
  benchmarks in the target `hub_id`.

Assignment policy:
- If category directory is empty for the hub, all processed products must end
  with `category_id = NULL`.
- Products with `category_assignment_source = "manual"` are immutable for this
  workflow and must not be changed (neither `category_id` nor
  `category_assignment_source`).
- If nearest neighbor cannot be converted to a valid category ID, skip that
  assignment and log `warn`.

## 6. Processing Workflow

1. Receive `ProductCategoryMatch(hub_id)` in `src/main.rs`.
2. Dispatch to `src/processing/category.rs` (new module).
3. Check processing flags for the target hub:
   - if any crawler or benchmark in `hub_id` has `processing = true`, log
     `warn` and skip.
4. Set `processing = true` for all crawler and benchmark rows in `hub_id`.
5. Load hub crawlers and products.
6. Load category-directory rows for the same hub.
7. Ensure category and product embeddings are available (generate missing).
8. Build cosine index over categories.
9. For each product, search top-1 category and persist `product.category_id`.
10. Set `processing = false` for all crawler and benchmark rows in `hub_id`.
11. Log completion metrics (products processed, matched, unmatched).

Failure behavior:
- Log and abort the current message on unrecoverable repository/embedding/index
  errors.
- Attempt to set all crawler and benchmark `processing` flags back to `false`
  for `hub_id` during finalization even when matching fails.
- Continue service loop for subsequent messages.

## 7. Repository Contract Changes (Planned)

New/extended traits should follow existing separation and be implemented in
`DieselRepository`:

- Category read methods (list by hub, optional get by id).
- Category embedding write method.
- Product category assignment write method.
- Optional clear/reset method for product category assignments by hub/crawler.
- Processing-guard methods:
  - check if any crawler or benchmark in a `hub_id` has `processing = true`,
  - set all crawlers in a `hub_id` `processing` to a target boolean value,
  - set all benchmarks in a `hub_id` `processing` to a target boolean value.

All DB operations must use Diesel query builder and schema from
`pushkind_dantes::schema`.

## 8. Prompting and Embeddings

Baseline:
- Reuse benchmark prompt style to keep embedding space behavior consistent.

Product prompt fields (expected):
- name, sku, category (legacy text), units, price, amount, description

Category prompt fields:
- category name only

Normalization:
- All generated vectors must be normalized to unit length before indexing and
  persistence, consistent with benchmark workflow.

## 9. Observability

Logging levels:
- `info`: message receipt, counts, lifecycle start/finish
- `warn`: invalid IDs/distances, skipped assignments
- `error`: repository/embedding/index failures that abort message processing

Suggested info metrics in logs:
- categories loaded
- products loaded
- embeddings generated/reused counts
- matched/unmatched counts
- skipped-because-processing-active count

## 10. Idempotency

Job is best-effort and replay-safe:
- Re-running for same `hub_id` should converge to the same assignments for
  unchanged data.
- Existing assignments can be overwritten by recomputed best match.

## 11. Testing Requirements

Unit tests:
- prompt formatting for category embeddings
- normalization and top-1 search behavior
- selection behavior for empty category directory

Integration tests:
- repository assignment + embedding persistence for categories/products
- end-to-end processing for a small seeded hub with deterministic expected
  matches
- guard behavior: job is skipped when any crawler/benchmark in `hub_id` has
  `processing = true`
- lifecycle behavior: all crawler/benchmark flags are set to `true` before
  matching and restored to `false` after completion (including failure paths),
  scoped to the target `hub_id`

## 12. Open Questions

- None currently.
