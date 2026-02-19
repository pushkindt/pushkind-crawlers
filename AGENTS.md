# AGENTS.md

This document provides guidance to AI code generators when working in this
repository. Follow these practices so new code matches the established
architecture and conventions.

## Project Context

`pushkind-crawlers` is a Rust 2024 Tokio-based background service. It listens
for `ZMQCrawlerMessage`s over ZeroMQ, hydrates shared domain models from the
`pushkind-dantes` crate, and coordinates crawler refreshes and benchmark
processing. The crate is organised into:

- `src/crawlers`: site-specific scrapers that implement the `WebstoreCrawler`
  trait and convert remote data into `pushkind_dantes::domain::product::NewProduct`.
- `src/processing`: message handlers that orchestrate crawl/benchmark workflows,
  enforce idempotency, and compose repository traits.
- `src/repository`: trait definitions and the Diesel-backed `DieselRepository`
  that bridges between SQLite tables (via `pushkind_dantes::schema`) and domain
  types.
- `src/main.rs`: the ZeroMQ listener that spawns Tokio tasks per message.

Favor keeping crawlers pure I/O, processing modules as the only place with
business rules, and repositories limited to database concerns.

## Development Commands

Use these commands to validate changes before committing:

**Build**
```bash
cargo build --all-features --verbose
```

**Run Tests**
```bash
cargo test --all-features --verbose
```

**Lint (Clippy)**
```bash
cargo clippy --all-features --tests -- -Dwarnings
```

**Format**
```bash
cargo fmt --all -- --check
```

`make check` runs `fmt`, `clippy`, and `test` in sequence.

## Coding Standards

- Write idiomatic async Rust; avoid blocking the Tokio runtime. Use the shared
  `reqwest::Client`, `tokio::sync::Semaphore`, and `futures` helpers already in
  the codebase.
- Do not use `unwrap`/`expect` in production paths. Propagate errors with
  `RepositoryResult<T>` and log meaningful context.
- Keep modules focused: add new crawlers under `src/crawlers`, processing logic
  under `src/processing`, and database accessors inside `src/repository`.
- Extend repository functionality by adding trait methods plus `DieselRepository`
  implementations so fakes/mocks remain swappable in tests.
- Prefer dependency injection via trait bounds (e.g., `R: ProductReader +
  ProductWriter`) and pass `DbPool`/traits into functions instead of relying on
  globals.
- Use strong domain types whenever available; prefer domain-specific value
  objects/newtypes over raw primitives (`String`, `i64`, etc.) in business
  logic and module interfaces.
- Normalise/sanitise external data inside crawler modules; keep transformation
  utilities (like `parse_amount_units`) reusable.
- When working with embeddings, convert between `Vec<f32>` and SQLite blobs
  using `bytemuck::cast_slice`, mirroring existing code.
- Document public APIs and behaviour that isn’t obvious from the signature.

## Database Guidelines

- Use Diesel’s query builder with schemas from `pushkind_dantes::schema`;
  do not write raw SQL.
- Obtain connections through `DieselRepository::conn()` and wrap multi-step
  changes in transactions when consistency matters.
- Map Diesel structs to domain models with explicit `From`/`Into`
  implementations provided by `pushkind-dantes`.
- Maintain referential integrity manually where required (e.g., clear
  `product_benchmark` rows before deleting products) and convert missing records
  into `RepositoryError::NotFound` instead of panicking.
- Store embeddings as blobs and normalise numeric data (prices, amounts) before
  persistence.

## Messaging and Crawling Guidelines

- Handle new ZeroMQ message variants inside `src/processing`; keep `main` limited
  to wiring and task supervision.
- When spawning Tokio tasks, clone the pool or repository handle and ensure all
  interior state is `Send + Sync`.
- For new crawlers, respect concurrency limits, reuse the shared HTTP client
  pattern, and emit `NewProduct` instances with consistent units/amount parsing.
- Log at `info` for lifecycle events, `warn` for retries or skip paths, and
  `error` when aborting work.

## Testing Expectations

- Add unit tests for new parsing logic, processing workflows, or utilities.
- Use `tests/common::TestDb` to create temporary SQLite databases for integration
  tests; avoid hard-coded file paths or shared state.
- Prefer `#[tokio::test]` for async scenarios and mock repository traits when
  verifying processing behaviour without hitting the database.
- Ensure new features have test coverage before opening a pull request and keep
  fixtures/data small to maintain fast test runs.

Following these practices keeps the crawler service reliable, composable, and
consistent with the existing architecture.
