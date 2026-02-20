# Plan: Product Category Match

Status: stable
Feature spec: `specs/features/product-category-match.md`  
ADR: `specs/decisions/0001-product-category-match.md`

## 1. Scope

Implement `ZMQCrawlerMessage::ProductCategoryMatch(hub_id)` processing that
assigns one best category-directory match to each product in the hub using
embeddings + cosine similarity, with hub-scoped crawler/benchmark
processing-flag guarding.

Source spec:
- `specs/features/product-category-match.md`

## 2. Assumptions

- `pushkind-dantes` already exposes:
  - category-directory domain type(s),
  - Diesel schema entries,
  - optional product `category_id` field.
- Existing product embedding field remains the same representation (SQLite blob
  from `Vec<f32>` via `bytemuck::cast_slice`).
- No new infrastructure/service dependencies are required.

## 3. Work Breakdown

1. Message wiring and processing module
- Add `src/processing/category.rs`.
- Export module from `src/processing/mod.rs`.
- Replace `todo!` in `src/main.rs` with handler invocation.

2. Repository trait surface
- Extend `src/repository/mod.rs` with category read/write traits and product
  category assignment methods.
- Add processing-guard trait methods for:
  - checking if any crawler/benchmark in `hub_id` has `processing = true`,
  - bulk-setting crawler `processing` values for `hub_id`,
  - bulk-setting benchmark `processing` values for `hub_id`.
- Keep trait boundaries aligned with current benchmark/crawler patterns.

3. Diesel repository implementation
- Add Diesel-backed implementations in new or existing repository modules.
- Implement list categories by hub, set category embedding, and set/clear
  product `category_id`.
- Ensure category assignment updates are conditional so
  `category_assignment_source = "manual"` rows are never overwritten by the
  automatic job.
- Respect transaction boundaries for multi-step updates where needed.

4. Matching pipeline implementation
- In `src/processing/category.rs`, implement:
  - preflight guard: abort if any crawler/benchmark in `hub_id` has
    `processing = true`,
  - set all crawler and benchmark `processing` flags to `true` for `hub_id`
    before matching,
  - embedding model initialization,
  - missing embedding generation for categories/products,
  - top-1 nearest neighbor search with cosine metric,
  - threshold filtering using crate-level `SIMILARITY_THRESHOLD` (same constant
    used by benchmark matching),
  - assignment persistence per product.
  - finalization that sets all crawler and benchmark `processing` flags to
    `false` for `hub_id` after completion, including error paths.
- Reuse helper patterns from `src/processing/benchmark.rs` where practical.

5. Logging and failure handling
- Add `info` start/finish logs and summary counts.
- Add `warn` when category matching is skipped because processing flags are
  already active in the target `hub_id`.
- Add `warn` for invalid IDs/skips.
- Add `error` for aborting failures, preserving service loop behavior.

6. Tests
- Add unit tests for prompt and nearest-neighbor behavior.
- Add integration tests for repository and processing workflow using
  `tests/common::TestDb`.
- Add integration tests for processing-flag guard semantics:
  - skip run if any crawler/benchmark in `hub_id` is already processing,
  - set all flags `true` for `hub_id` before matching starts,
  - restore all flags `false` for `hub_id` on both success and failure.

7. Documentation updates
- Update `SPEC.md` message contract and processing sections after implementation.
- Add ADR only if implementation introduces a new cross-cutting architecture
  pattern beyond existing processing/repository design.

## 4. Validation Checklist

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-features --tests -- -Dwarnings
cargo test --all-features --verbose
```

Success criteria:
- Service handles `ProductCategoryMatch` without panic/todo.
- Category matching does not start when any crawler/benchmark in the target
  `hub_id` has `processing=true` at message receipt.
- Category matching sets all crawler/benchmark processing flags `true` for
  `hub_id` before work and resets all to `false` for `hub_id` when done.
- Missing embeddings are backfilled and persisted.
- Products receive deterministic top-1 category assignments for fixed fixtures.
- Products marked with `category_assignment_source = "manual"` remain unchanged
  after automatic matching runs.
- Existing crawler and benchmark flows remain green.

## 5. Risks and Mitigations

- Large category/product sets increase embedding/index cost.
  - Mitigation: keep per-message in-memory structures bounded to hub scope and
    reuse embedder instance within run.
- Domain model mismatch with assumptions from `pushkind-dantes`.
  - Mitigation: adapt prompt/trait fields after inspecting actual types.
- Processing flags could remain `true` after unexpected failure.
  - Mitigation: implement explicit finalization path that always attempts flag
    reset and logs reset failures at `error`.

## 6. Pending Decisions

- None currently.
