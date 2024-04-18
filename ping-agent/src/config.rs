use once_cell::sync::Lazy;
use std::sync::Mutex;
use crate::PulsarConnectionData;
use crate::Warp10ConnectionData;

pub static PULSAR_CONNECTION_DATA: Lazy<Mutex<PulsarConnectionData>> = Lazy::new(|| {
    Mutex::new(PulsarConnectionData {
        pulsar_address: std::env::var("PULSAR_ADDRESS").expect("PULSAR_ADDRESS must be set"),
        pulsar_token: std::env::var("PULSAR_TOKEN").expect("PULSAR_TOKEN must be set"),
        pulsar_tenant: std::env::var("PULSAR_TENANT").expect("PULSAR_TENANT must be set"),
        pulsar_namespace: std::env::var("PULSAR_NAMESPACE").expect("PULSAR_NAMESPACE must be set"),
        pulsar_topic: std::env::var("PULSAR_TOPIC").expect("PULSAR_TOPIC must be set"),
    })
});

pub static WARP10_CONNECTION_DATA: Lazy<Mutex<Warp10ConnectionData>> = Lazy::new(|| {
    Mutex::new(Warp10ConnectionData {
        warp10_address: std::env::var("WARP10_ADDRESS").expect("WARP10_ADDRESS must be set"),
        warp10_token: std::env::var("WARP10_TOKEN").expect("WARP10_TOKEN must be set"),
    })
});

pub static REDIS_URL: Lazy<String> = Lazy::new(|| {
    let redis_host = std::env::var("REDIS_HOST").expect("REDIS_HOST must be set");
    let redis_port = std::env::var("REDIS_PORT").expect("REDIS_PORT must be set");
    let redis_password = std::env::var("REDIS_PASSWORD").expect("REDIS_PASSWORD must be set");
    format!("redis://:{}@{}:{}/", redis_password, redis_host, redis_port)
});