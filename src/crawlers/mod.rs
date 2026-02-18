use async_trait::async_trait;
use pushkind_dantes::domain::product::NewProduct;
use rand::distr::{Alphanumeric, SampleString};
use regex::Regex;
use thiserror::Error;

pub mod gutenberg;
pub mod rusteaco;
pub mod tea101;
pub mod teanadin;
pub mod wintergreen;

#[derive(Error, Debug)]
pub enum CrawlerError {
    #[error("Failed to create a crawler: {0}")]
    Build(String),
}

pub type CrawlerResult<T> = Result<T, CrawlerError>;

/// An abstraction over web store crawlers that produce [`NewProduct`]s.
#[async_trait]
pub trait WebstoreCrawler: Send + Sync {
    /// Crawls the target site and returns every product discovered.
    async fn get_products(&self) -> Vec<NewProduct>;

    /// Fetches product information from a single URL.
    ///
    /// Some pages may describe multiple product variants, therefore the
    /// implementation returns a collection of [`NewProduct`]s.
    async fn get_product(&self, url: &str) -> Vec<NewProduct>;
}

fn parse_amount_units(input: &str) -> (f64, String) {
    let default_amount = 1.0;
    let default_units = "шт".to_string();

    let trimmed = input.trim_start_matches('/').trim_start();

    // Regex to capture number (with optional decimal) and optional unit
    let re = Regex::new(r"(?i)^\s*(\d+(?:[.,]\d+)?)([a-zа-я%]*)\s*$").unwrap();

    if let Some(caps) = re.captures(trimmed) {
        let amount_str = caps.get(1).unwrap().as_str().replace(',', ".");
        let amount = amount_str.parse::<f64>().unwrap_or(default_amount);
        let units = caps
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or(default_units.clone());
        return (
            amount,
            if units.is_empty() {
                default_units
            } else {
                units
            },
        );
    }

    // Fallback: split by spaces like before
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() >= 2 {
        let amount_str = tokens[tokens.len() - 2].replace(',', ".");
        let amount = amount_str.parse::<f64>().unwrap_or(default_amount);
        let units = tokens.last().unwrap().to_string();
        (amount, units)
    } else if tokens.len() == 1 {
        let amount = tokens[0]
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(default_amount);
        (amount, default_units)
    } else {
        (default_amount, default_units)
    }
}

fn build_reqwest_client() -> CrawlerResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(Alphanumeric.sample_string(&mut rand::rng(), 16))
        .build()
        .map_err(|e| CrawlerError::Build(e.to_string()))
}
