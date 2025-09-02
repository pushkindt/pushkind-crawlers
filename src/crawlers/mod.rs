use async_trait::async_trait;
use pushkind_common::domain::dantes::product::NewProduct;

pub mod gutenberg;
pub mod rusteaco;
pub mod tea101;

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
    // Default values
    let default_amount = 1.0;
    let default_units = "шт".to_string();

    // Remove optional "/" and leading spaces
    let trimmed = input.trim_start_matches('/').trim_start();

    // Split into tokens by whitespace
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();

    if tokens.len() >= 2 {
        // Take the last two tokens
        let amount_str = tokens[tokens.len() - 2].replace(',', ".");
        let amount = amount_str.parse::<f64>().unwrap_or(default_amount);
        let units = tokens.last().unwrap().to_string();
        (amount, units)
    } else if tokens.len() == 1 {
        // Single token (could be amount or unit)
        let amount = tokens[0]
            .replace(',', ".")
            .parse::<f64>()
            .unwrap_or(default_amount);
        (amount, default_units)
    } else {
        // Empty input
        (default_amount, default_units)
    }
}
