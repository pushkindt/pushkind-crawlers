# pushkind-crawlers Specification

## 1. Purpose

`pushkind-crawlers` is a Rust 2024 Tokio service that:
- accepts crawler and benchmark jobs over ZeroMQ,
- crawls external tea webstores and normalizes products into `NewProduct`,
- persists products/metadata into SQLite via Diesel repository traits,
- computes text embeddings and benchmark-to-product associations.

This document specifies the current implemented behavior in this repository.

## 2. System Scope

In scope:
- ZeroMQ message intake (`PULL` socket).
- Crawler orchestration for full refresh and targeted product updates.
- Benchmark embedding and similarity matching.
- Repository read/write behavior for crawlers, products, benchmarks.

Out of scope:
- HTTP API or UI.
- Scheduling logic (jobs are externally produced and pushed to ZeroMQ).
- Database schema ownership (comes from `pushkind_dantes` / `pushkind_common`).

## 3. High-Level Architecture

Flow:
1. `src/main.rs` loads config, initializes logging, DB pool, and ZeroMQ socket.
2. For each incoming message, a Tokio task is spawned.
3. Processing handlers (`src/processing`) execute business workflows.
4. Site crawlers (`src/crawlers`) fetch and parse remote pages.
5. Repository traits (`src/repository`) persist/retrieve domain entities via Diesel.

Primary layers:
- `src/main.rs`: wiring and supervision.
- `src/processing/*`: workflow orchestration and branching.
- `src/crawlers/*`: external I/O + parsing + normalization.
- `src/repository/*`: DB operations and trait boundaries.

## 4. Runtime and Configuration

Startup behavior:
- `.env` is loaded via `dotenvy`.
- Logging is initialized via `env_logger` with default filter `info`.
- `APP_ENV` selects config overlay (`local` fallback).

Config sources (merge order):
1. `config/default.yaml`
2. `config/{APP_ENV}.yaml` (optional)
3. Environment variables with `APP_` prefix

Current config model (`ServerConfig`):
- `database_url: String`
- `zmq_crawlers_sub: String`

Default config values:
- `database_url: app.db`
- `zmq_crawlers_sub: tcp://127.0.0.1:5550`

Effective env override names:
- `APP_DATABASE_URL`
- `APP_ZMQ_CRAWLERS_SUB`

## 5. Message Contract and Dispatch

Incoming bytes are decoded as `pushkind_dantes::domain::zmq::ZMQCrawlerMessage`.

Dispatch:
- `ZMQCrawlerMessage::Crawler(crawler_msg)` -> `process_crawler_message`
- `ZMQCrawlerMessage::Benchmark(benchmark_id)` -> `process_benchmark_message`

Operational behavior:
- Parse failures are logged and skipped.
- Receive errors are logged and loop continues.
- Each valid message runs in a separate Tokio task with its own `DieselRepository`.

Observed JSON examples from test client:
- `{"Crawler":{"Selector":"wintergreen"}}`
- `{"Crawler":{"SelectorProducts":["teanadin",["https://..."]]}}`
- `{"Benchmark":1}`

## 6. Crawler Processing Specification

Handler: `process_crawler_message<R>(msg, repo)` where
`R: CrawlerReader + CrawlerWriter + ProductWriter`.

Input modes:
- Full run: `Selector(selector)` -> crawl entire catalog.
- Partial run: `SelectorProducts((selector, urls))` -> update only provided URLs.

Selector to crawler implementation mapping:
- `rusteaco` -> `WebstoreCrawlerRusteaco::new(5, crawler_id)`
- `101tea` -> `WebstoreCrawler101Tea::new(5, crawler_id)`
- `gutenberg` -> `WebstoreCrawlerGutenberg::new(5, crawler_id)`
- `teanadin` -> `WebstoreCrawlerTeanadin::new(1, crawler_id)`
- `wintergreen` -> `WebstoreCrawlerWintergreen::new(1, crawler_id)`

Workflow:
1. Load crawler row by selector from repository.
2. If crawler is already `processing=true`, log warning and exit.
3. Set `processing=true`.
4. If full run:
- delete existing crawler products,
- crawl all products with `get_products`,
- insert with `create_products`.
5. If partial run:
- fetch each URL via `get_product`,
- flatten variant results,
- upsert with `update_products`.
6. Update crawler stats (`updated_at`, `processing=false`, `num_products`).

## 7. Crawler Subsystem Specification

### 7.1 Shared crawler behavior

All webstore crawlers implement trait:
- `async fn get_products(&self) -> Vec<NewProduct>`
- `async fn get_product(&self, url: &str) -> Vec<NewProduct>`

Shared implementation patterns:
- `reqwest::Client` per crawler instance.
- `Semaphore` caps concurrent HTTP requests.
- Crawl strategy: category links -> paginated listing links -> product links -> product pages.
- Product URLs are deduplicated with `HashSet`.
- Final collected products are deduplicated by `NewProduct.url`.

Shared normalization helpers:
- `build_new_product(...) -> Option<NewProduct>`
- `parse_amount_units(&str) -> (f64, String)`
- `build_reqwest_client()` with randomized alphanumeric user-agent.

Validation in `build_new_product`:
- Converts primitive values into domain types (`ProductSku`, `ProductName`, etc.).
- Rejects invalid values and logs warnings.
- Trims empty optional strings to `None`.
- Filters invalid image URLs.

`parse_amount_units` behavior:
- Supports strings like `/100 г`, `0.5кг`, `100`.
- Default fallback is `(1.0, "шт")`.
- Comma decimal separators are normalized to dots.

### 7.2 Site-specific extraction

`gutenberg`:
- Base: `https://gutenberg.ru/`
- Categories: `ul.menu-type-1 li a`
- Pagination param: `page`
- Product links: `div.item-title > a`
- Product fields from selectors:
  - name: `h1#pagetitle`
  - description: `div[itemprop='description']`
  - sku: `span.article__value`
  - price: `span.price_value`
  - amount/units: `span.price_measure` (parsed via `parse_amount_units`)

`101tea`:
- Base: `https://101tea.ru/`
- Categories: `a.catalog-nav__link`
- Pagination param: `PAGEN_1`
- Product links: `div.product-card__info-bottom > a`
- Product fields from selectors:
  - name: `h1`
  - description: `div.catalog-table_content-item_about_product`
  - sku: `div.product_art span:nth-child(2)`
  - price: `span.js-price-val`
  - units: `span.product-card__calculus-unit`
  - amount: `span.js-product-calc-value`

`rusteaco`:
- Base: `https://shop.rusteaco.ru/`
- Categories: `a.header__collections-link`
- Pagination param: `page`
- Product links: `div.product-preview__title > a`
- Product page supports variant JSON in `form.product[data-product-json]`.
- JSON variants produce multiple products (URL includes `#{sku}` suffix).
- Fallback non-JSON parsing supported (single SKU path).

`teanadin`:
- Base: `https://teanadin.ru/`
- Categories: `ul.header-menu__wide-submenu li a`
- Pagination param: `PAGEN_2`
- Product links: `div.catalog-block__info-title > a`
- Amount/units from `span.sku-props__js-size` with `parse_amount_units`.
- Images from `img.detail-gallery-big__picture[data-src]` (joined to base URL).

`wintergreen`:
- Base: `https://wintergreen.ru/`
- Categories: `a.menu-navigation__sections-item-link`
- Pagination param: `PAGEN_1`
- Product links: `div.item-title > a`
- Images from `img.product-detail-gallery__picture[data-src]`.

## 8. Repository Specification

Implementation: `DieselRepository { pool: DbPool }`.

Trait boundaries:
- `ProductReader`: `list_products`
- `ProductWriter`: `create_products`, `update_products`, `set_product_embedding`, `delete_products`
- `CrawlerReader`: `get_crawler`, `list_crawlers`
- `CrawlerWriter`: `update_crawler_stats`, `set_crawler_processing`
- `BenchmarkReader`: `get_benchmark`
- `BenchmarkWriter`: benchmark embedding/association/processing/stats methods

Key persistence behavior:
- `create_products` inserts one-by-one in a transaction and writes images.
- `update_products` upserts on `(crawler_id, url)`, updates `updated_at`, rewrites images.
- Product image replacement deletes old image rows then inserts current set.
- `delete_products` transactionally deletes related `product_images` and `product_benchmark` before product deletion.
- Embeddings are stored as SQLite BLOB (`Vec<f32>` <-> bytes via `bytemuck::cast_slice`).
- `update_*_stats` methods set `processing=false`, update timestamps, and count associated products.

## 9. Benchmark Processing Specification

Handler: `process_benchmark_message<R>(benchmark_id, repo)` where
`R: BenchmarkReader + BenchmarkWriter + ProductReader + ProductWriter + CrawlerReader`.

Workflow:
1. Load benchmark by ID.
2. If benchmark already processing, warn and exit.
3. Set benchmark `processing=true`.
4. Run `process_benchmark(benchmark, &repo)`.
5. Always call `update_benchmark_stats` afterward.

`process_benchmark` core logic:
1. Initialize `fastembed::TextEmbedding` with `MultilingualE5Large`.
2. Ensure benchmark embedding exists:
- if stored embedding exists, load from blob,
- else build prompt text and generate normalized embedding, then persist.
3. Load all crawlers for benchmark hub.
4. Remove all previous benchmark-product associations.
5. For each crawler:
- load products,
- ensure each product embedding exists (generate/persist if missing),
- perform ANN search with `usearch` cosine index over crawler products,
- take top 10 neighbors.
6. Convert `usearch` distance to similarity via `similarity = 1.0 - distance`.
7. Apply threshold `similarity >= 0.8`.
8. Insert valid `(benchmark_id, product_id, similarity_distance)` associations.

Prompt template used for embeddings:
- Name
- SKU
- Category
- Units
- Price
- Amount
- Description

## 10. Logging and Error Semantics

Logging levels:
- `info`: lifecycle events (message received, per-crawler benchmark processing, finished events).
- `warn`: concurrent processing guard, invalid converted IDs/distances.
- `error`: configuration failures, parsing failures, HTTP failures, DB failures, embedding/search failures.

Failure behavior:
- Startup config/DB/ZeroMQ bind failures terminate process (`exit(1)`).
- Runtime message/processing failures are logged; service keeps listening.

## 11. Performance and Concurrency Characteristics

- Message-level parallelism: one Tokio task per valid ZeroMQ message.
- Crawler HTTP parallelism: bounded by site-specific semaphore size.
- Within a crawl run, category/page/product fetch operations use `futures::join_all`.
- Benchmark matching builds an in-memory `usearch` index per crawler product set.

## 12. Testing Status

Current tests in repository:
- `src/processing/benchmark.rs`: prompt formatting unit test.
- `src/crawlers/rusteaco.rs`: variant conversion and amount/unit defaulting tests.
- `tests/db.rs` + `tests/common/mod.rs`: temporary DB lifecycle helper test.

No broad integration coverage currently exists for:
- end-to-end ZeroMQ message processing,
- crawler HTML parsing against fixtures,
- repository CRUD behavior across all methods,
- benchmark association threshold logic.

## 13. Operational Commands

Build:
```bash
cargo build --all-features --verbose
```

Test:
```bash
cargo test --all-features --verbose
```

Lint:
```bash
cargo clippy --all-features --tests -- -Dwarnings
```

Format:
```bash
cargo fmt --all -- --check
```

Combined check:
```bash
make check
```

## 14. Known Current Limitations

- In crawler processing, some early-return error paths after `set_crawler_processing(true)` can skip `update_crawler_stats`, leaving `processing=true` until later manual/automated correction.
- Crawler HTTP requests do not currently implement explicit retry/backoff policy.
- Selector-based HTML parsing is tightly coupled to current store markup and may break when sites change structure.
- Benchmark embedding generation is performed product-by-product and can be costly for large catalogs.

## 15. Idempotency and Duplicate Messages

Current behavior is intentionally best-effort and not strictly idempotent.

- Duplicate ZeroMQ messages are allowed and may trigger duplicate work.
- Processing guards (`processing=true`) prevent some concurrent overlap per crawler/benchmark but do not provide message-level deduplication guarantees.
- There is no message ID or durable dedupe store in this service today.

Operational interpretation:
- At-least-once style execution with possible replay/duplication side effects is acceptable in the current design.

## 16. Resource Envelope (Current State)

The service currently does not define formal hard resource limits in the spec for:
- maximum catalog/product count per crawler run,
- peak embedding memory usage,
- worst-case `usearch` index size during benchmark matching.

Practical constraints are currently implicit:
- crawler HTTP concurrency is semaphore-bounded,
- message handling is task-based and unbounded by queue depth in-process,
- embedding/index workloads scale with per-crawler product volume.

## 17. Cross-Repository Responsibility Boundary

Boundary with `pushkind-dantes` is explicit:
- `pushkind-dantes` defines domain models, schema, and semantic intent.
- `pushkind-crawlers` defines runtime execution: message handling, crawling, embedding, and persistence orchestration.

This service should not redefine domain rules owned by `pushkind-dantes`; it should only implement operational workflows against those contracts.
