pub mod api;
pub mod db;

use api::ApiHandler;
use db::DbHandler;
use env_logger::{Builder as Logger, Env};
use log::{error, info};
use std::{process, str::FromStr, sync::Arc};

/// Get env var as string or panic
pub fn env_get(env: &'static str) -> String {
    let env_panic = |e| {
        panic!("{env} is not set ({})", e);
    };

    std::env::var(env).map_err(env_panic).unwrap()
}

/// Get env var as number or panic, with a default number
pub fn env_get_num<T: FromStr>(env: &'static str, other: T) -> T {
    let env_parse_panic = |v| {
        panic!("can't parse {env} ({v})");
    };
    match std::env::var(env) {
        Ok(v) => v.parse::<T>().map_err(|_| env_parse_panic(v)).unwrap(),
        Err(_) => other,
    }
}

/// Start logger with default log level : info (overridden by env var LOG_LEVEL)
pub fn init_logger() {
    let env = Env::new().filter_or("LOG_LEVEL", "info");
    Logger::from_env(env).init();
}

#[tokio::main]
async fn main() {
    let address = env_get("ADDRESS");
    let port = env_get("PORT");
    let postgresql_uri = env_get("DATABASE_URL");

    init_logger();

    let db = DbHandler::connect(&postgresql_uri)
        .await
        .unwrap_or_else(|| {
            error!("Could not open database at {}", postgresql_uri);
            process::exit(1);
        });

    let app = api::routes::app(Arc::new(ApiHandler { db }));

    let listener = tokio::net::TcpListener::bind(format!("{address}:{port}"))
        .await
        .unwrap();

    info!("Starting api at http://{address}:{port}...");
    _ = axum::serve(listener, app).await
}
