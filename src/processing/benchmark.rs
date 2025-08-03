use pushkind_common::db::DbPool;
use pushkind_common::domain::benchmark::Benchmark;
use pushkind_common::domain::product::Product;

fn prompt(p: &Product) -> String {
    format!(
        "Name: {}\nSKU: {}\nCategory: {}\nUnits: {}\nPrice: {}\nAmount: {}\nDescription: {}",
        p.name,
        p.sku,
        p.category.as_deref().unwrap_or(""),
        p.units.as_deref().unwrap_or(""),
        p.price,
        p.amount.unwrap_or(0.0),
        p.description.as_deref().unwrap_or(""),
    )
}

pub async fn process_benchmark_message(msg: i32, db_pool: &DbPool) {
    log::info!("Received benchmark: {msg:?}");

    log::info!("Finished processing benchmar: {msg}");
}
