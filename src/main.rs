use std::env;
use std::sync::Arc;

use pushkind_common::db::establish_connection_pool;
use pushkind_common::models::zmq::dantes::ZMQMessage;

use pushkind_crawlers::processing::benchmark::process_benchmark_message;
use pushkind_crawlers::processing::crawler::process_crawler_message;
use pushkind_crawlers::repository::DieselRepository;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let database_url = env::var("DATABASE_URL").unwrap_or("app.db".to_string());
    let pool = match establish_connection_pool(&database_url) {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("Failed to establish database connection: {e}");
            std::process::exit(1);
        }
    };
    let pool = Arc::new(pool);

    let zmq_address =
        env::var("ZMQ_ADDRESS").unwrap_or_else(|_| "tcp://127.0.0.1:5555".to_string());
    let context = zmq::Context::new();
    let responder = context.socket(zmq::PULL).expect("Cannot create zmq socket");
    responder
        .bind(&zmq_address)
        .expect("Cannot bind to zmq port");

    loop {
        let msg = responder.recv_bytes(0).unwrap();
        match serde_json::from_slice::<ZMQMessage>(&msg) {
            Ok(parsed) => {
                let pool_clone = Arc::clone(&pool);
                tokio::spawn(async move {
                    let repo = DieselRepository::new(&pool_clone);
                    match parsed {
                        ZMQMessage::Crawler(crawler) => {
                            process_crawler_message(crawler, repo).await
                        }
                        ZMQMessage::Benchmark(benchmark) => {
                            process_benchmark_message(benchmark, repo).await
                        }
                    }
                });
            }
            Err(e) => log::error!("Failed to parse JSON: {e}"),
        }
    }
}
