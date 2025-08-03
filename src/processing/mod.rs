use serde::Deserialize;

pub mod benchmark;
pub mod crawler;

#[derive(Deserialize, Debug)]
pub enum ZMQMessage {
    Crawler(CrawlerSelector),
    Benchmark(i32),
}

#[derive(Deserialize, Debug)]
pub enum CrawlerSelector {
    Selector(String),
    SelectorProducts((String, Vec<String>)),
}
