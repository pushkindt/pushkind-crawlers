use serde::Deserialize;

pub mod benchmark;
pub mod crawler;

/// Messages received over ZMQ to control crawlers or run benchmarks.
///
/// - `Crawler` requests execution of a crawler described by [`CrawlerSelector`].
/// - `Benchmark` triggers a benchmark run with the provided benchmark_id.
#[derive(Deserialize, Debug)]
pub enum ZMQMessage {
    /// Run the specified crawler.
    Crawler(CrawlerSelector),
    /// Execute benchmarks with the given number of iterations.
    Benchmark(i32),
}

/// Selects a crawler and optionally a list of product URLs to crawl.
///
/// - `Selector` chooses a crawler by name.
/// - `SelectorProducts` specifies a crawler and products to fetch.
#[derive(Deserialize, Debug)]
pub enum CrawlerSelector {
    /// Run the named crawler.
    Selector(String),
    /// Run the named crawler with the provided product URLs.
    SelectorProducts((String, Vec<String>)),
}
