use std::str::FromStr;
use env_logger::{Builder as Logger, Env};
use log::{error, info};
use http::warp10::{PulsarConnectionData, PulsarHttpSource, Warp10Client, Warp10ConnectionData, Warp10HttpSink};
use crate::http::warp10::pulsar_http_topic;

pub mod http;
mod warp10;

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
    let pulsar_address = env_get("PULSAR_ADDRESS");
    let pulsar_token = env_get("PULSAR_TOKEN");
    let pulsar_tenant = env_get("PULSAR_TENANT");
    let pulsar_namespace = env_get("PULSAR_NAMESPACE");

    let subscription_uuid = env_get("SUBSCRIPTION_ID");

    init_logger();

    let pulsar_connection_data = PulsarConnectionData {
        pulsar_address,
        pulsar_token,
        pulsar_tenant,
        pulsar_namespace,
        subscription_uuid,
    };

    let warp10_address = env_get("WARP10_ADDRESS");
    let warp10_token = env_get("WARP10_TOKEN");

    let warp10_connection_data = Warp10ConnectionData {
        warp10_address,
        warp10_token,
    };

    info!(
        "Connecting to pulsar topic {}...",
        pulsar_http_topic(&pulsar_connection_data)
    );

    let pulsar_source = match PulsarHttpSource::new(&pulsar_connection_data).await {
        Some(pc) => {
            info!("Connected to pulsar topic !");
            pc
        }
        None => {
            error!("Failed to connect to pulsar topic");
            std::process::exit(1);
        }
    };

    info!("Connecting to warp10...",);
    let warp10_client = match Warp10Client::new(warp10_connection_data) {
        Some(pc) => {
            info!("Connected to warp10 !");
            pc
        }
        None => {
            error!("Failed to connect to warp10");
            std::process::exit(1);
        }
    };

    let warp10_http_sink = Warp10HttpSink::new(warp10_client, pulsar_source);

    warp10_http_sink.run().await;
}
