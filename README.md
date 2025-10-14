# pushkind-crawlers

`pushkind-crawlers` is a Rust 2024 background service that keeps the Pushkind
pricing dataset fresh. It listens for crawl/benchmark jobs over ZeroMQ,
downloads product catalogues from supported retailers, stores the results in
SQLite via Diesel, and computes text embeddings to match products to internal
benchmarks.

## Why it exists

The Pushkind platform compares marketplace products against curated benchmark
items. This service automates the data collection loop:

- Receives `ZMQCrawlerMessage`s over a ZeroMQ `PULL` socket.
- Refreshes catalogues for retailer-specific crawlers on demand or for select
  product URLs.
- Generates multilingual embeddings for benchmarks and crawled products, then
  links close matches for downstream analytics.
- Keeps crawl/benchmark metadata up to date so other services can act on the
  freshest information.

## Architecture

- **Entry point** (`src/main.rs`): initialises logging, reads `.env`, connects to
  SQLite, binds the ZeroMQ listener, and spawns a Tokio task per message.
- **Processing layer** (`src/processing`): orchestrates crawl or benchmark flows
  by composing repository traits, guarding against concurrent runs, and logging
  progress.
- **Crawler implementations** (`src/crawlers`): async scrapers for each retailer
  that emit `pushkind_common::domain::dantes::product::NewProduct`.
- **Repository layer** (`src/repository`): defines trait-based boundaries and the
  Diesel-backed `DieselRepository` that interacts with tables from
  `pushkind-common`.
- **Tests** (`tests/`): use `tests/common::TestDb` for temporary SQLite databases
  and lay groundwork for integration scenarios.

```
ZeroMQ → processing::{crawler,benchmark} → repository traits → SQLite
                             │
                         crawlers::*
```

## Features

- Push/pull job handling over ZeroMQ with Tokio task fan-out.
- Retailer-specific crawlers (Rusteaco, 101Tea, Gutenberg) with concurrent HTTP
  fetching, HTML parsing, and data normalisation helpers such as
  `parse_amount_units`.
- Repository layer that encapsulates all Diesel queries, casting between blobs
  and `Vec<f32>` for embedding storage.
- Embedding workflow powered by `fastembed` and `usearch` to find top-matching
  products per benchmark.
- Shared domain models, schema definitions, and utilities via the
  `pushkind-common` crate to stay consistent with other services.

## Repository layout

```
├── src/
│   ├── main.rs            # ZeroMQ listener and Tokio runtime bootstrap
│   ├── crawlers/          # Retailer-specific implementations of WebstoreCrawler
│   ├── processing/        # Message handlers and orchestration logic
│   └── repository/        # Trait definitions and Diesel-backed implementations
├── tests/                 # Integration test harnesses and helpers
├── test_client.py         # Simple ZeroMQ PUSH client for manual testing
└── Makefile               # Convenience target for formatting, linting, testing
```

## Getting started

1. Install Rust 2024 toolchain, SQLite, and ZeroMQ development headers.
2. Clone `pushkind-common` dependencies by running `cargo fetch`.
3. Create a `.env` file (or export env vars):
   - `DATABASE_URL` – SQLite connection string (defaults to `app.db`).
   - `ZMQ_CRAWLER` – ZeroMQ bind address for incoming jobs (`tcp://127.0.0.1:5555`
     by default).
4. Start ZeroMQ producers or use `python test_client.py` to send sample jobs.

```bash
cargo run --release
```

The service logs when crawlers and benchmarks start/finish, and any issues with
fetching, parsing, or database updates.

## Development workflow

- Format: `cargo fmt --all`
- Lint: `cargo clippy --all-features --tests -- -Dwarnings`
- Tests: `cargo test --all-features`
- One-shot helper: `make check`

When adding new functionality, follow the guidelines in [`AGENTS.md`](AGENTS.md)
to keep code consistent with the existing architecture.
