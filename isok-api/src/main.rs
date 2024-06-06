use std::str::FromStr;

use env_logger::{Builder as Logger, Env};
use log::{error, info};

use api::ApiHandler;
use db::DbHandler;

use crate::{
    api::ApiHandlerState,
    pulsar::{PulsarClient, PulsarConnectionData},
};

pub mod api;
pub mod db;
pub mod pulsar;

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

    let pulsar_address = env_get("PULSAR_ADDRESS");
    let pulsar_token = env_get("PULSAR_TOKEN");
    let pulsar_tenant = env_get("PULSAR_TENANT");
    let pulsar_namespace = env_get("PULSAR_NAMESPACE");
    let pulsar_topic = env_get("PULSAR_TOPIC");

    init_logger();

    let pulsar_connection_data = PulsarConnectionData {
        pulsar_address,
        pulsar_token,
        pulsar_tenant,
        pulsar_namespace,
        pulsar_topic,
    };

    info!("Connecting to database...");
    let db = match DbHandler::connect(&postgresql_uri).await {
        Some(db) => {
            info!("Connected to database !");
            db
        }
        None => {
            error!("Failed to connect to database");
            std::process::exit(1);
        }
    };

    info!(
        "Connecting to pulsar topic {}...",
        pulsar::pulsar_link(&pulsar_connection_data)
    );
    let pulsar_client = match PulsarClient::new(pulsar_connection_data).await {
        Some(pc) => {
            info!("Connected to pulsar topic !");
            pc
        }
        None => {
            error!("Failed to connect to pulsar topic");
            std::process::exit(1);
        }
    };

    let app = api::routes::app(ApiHandlerState::new(ApiHandler {
        db,
        pulsar_client: pulsar_client,
    }));

    let listener = tokio::net::TcpListener::bind(format!("{address}:{port}"))
        .await
        .unwrap();

    info!("Starting api at http://{address}:{port}...");
    _ = axum::serve(listener, app).await
}
