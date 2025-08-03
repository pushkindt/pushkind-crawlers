use pushkind_common::db::DbPool;

pub async fn process_benchmark_message(msg: i32, db_pool: &DbPool) {
    log::info!("Received benchmark: {msg:?}");

    log::info!("Finished processing benchmar: {msg}");
}
