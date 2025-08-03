use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Product {
    pub sku: String,
    pub name: String,
    pub price: f32,
    pub category: String,
    pub units: String,
    pub amount: f32,
    pub description: String,
    pub url: String,
}
