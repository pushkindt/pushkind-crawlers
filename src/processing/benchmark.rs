use pushkind_common::db::DbPool;

fn prompt(
    name: &str,
    sku: &str,
    category: &str,
    units: &str,
    price: f64,
    amount: f64,
    description: &str,
) -> String {
    format!(
        "Name: {name}\nSKU: {sku}\nCategory: {category}\nUnits: {units}\nPrice: {price}\nAmount: {amount}\nDescription: {description}",
    )
}

pub async fn process_benchmark_message(msg: i32, db_pool: &DbPool) {
    log::info!("Received benchmark: {msg:?}");

    log::info!("Finished processing benchmar: {msg}");
}
