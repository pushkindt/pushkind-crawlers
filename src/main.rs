use std::env;

use config::Config;
use dotenvy::dotenv;
use pushkind_common::db::establish_connection_pool;
use pushkind_crawlers::models::config::ServerConfig;
use pushkind_crawlers::processing::benchmark::process_benchmark_message;
use pushkind_crawlers::processing::crawler::process_crawler_message;
use pushkind_crawlers::repository::DieselRepository;
use pushkind_dantes::domain::zmq::ZMQCrawlerMessage;

/// Entry point for the crawler service.
#[tokio::main]
async fn main() {
    // Load environment variables from `.env` in local development.
    dotenv().ok();
    // Initialize logger with default level INFO if not provided.
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Select config profile (defaults to `local`).
    let app_env = env::var("APP_ENV").unwrap_or_else(|_| "local".into());

    let settings = Config::builder()
        // Add `./config/default.yaml`
        .add_source(config::File::with_name("config/default"))
        // Add environment-specific overrides
        .add_source(config::File::with_name(&format!("config/{}", app_env)).required(false))
        // Add settings from the environment (with a prefix of APP)
        .add_source(config::Environment::with_prefix("APP"))
        .build();

    let settings = match settings {
        Ok(settings) => settings,
        Err(err) => {
            log::error!("Error loading settings: {}", err);
            std::process::exit(1);
        }
    };

    let server_config = match settings.try_deserialize::<ServerConfig>() {
        Ok(server_config) => server_config,
        Err(err) => {
            log::error!("Error loading server config: {}", err);
            std::process::exit(1);
        }
    };

    let pool = match establish_connection_pool(&server_config.database_url) {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("Failed to establish database connection: {e}");
            std::process::exit(1);
        }
    };

    let context = zmq::Context::new();
    let responder = match context.socket(zmq::PULL) {
        Ok(socket) => socket,
        Err(err) => {
            log::error!("Cannot create zmq socket: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = responder.bind(&server_config.zmq_crawlers_sub) {
        log::error!(
            "Cannot bind to zmq port {}: {err}",
            server_config.zmq_crawlers_sub
        );
        std::process::exit(1);
    }

    loop {
        let msg = match responder.recv_bytes(0) {
            Ok(msg) => msg,
            Err(err) => {
                log::error!("Failed to receive ZMQ message: {err}");
                continue;
            }
        };
        match serde_json::from_slice::<ZMQCrawlerMessage>(&msg) {
            Ok(parsed) => {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    let repo = DieselRepository::new(pool_clone);
                    match parsed {
                        ZMQCrawlerMessage::Crawler(crawler) => {
                            process_crawler_message(crawler, repo).await
                        }
                        ZMQCrawlerMessage::Benchmark(benchmark) => {
                            process_benchmark_message(benchmark, repo).await
                        }
                    }
                });
            }
            Err(e) => log::error!("Failed to parse JSON: {e}"),
        }
    }
}
