/// http ping module
pub mod http;
/// icmp ping module
pub mod icmp;
/// Job scheduling module
pub mod job;
/// [MagicPool](crate::magic_pool::MagicPool)'s module
pub mod magic_pool;
/// pulsar related stuff
pub mod pulsar_client;
/// tcp ping module
pub mod tcp;
/// warp10 related stuff
pub mod warp10;
//redis ping module
pub mod redis;
//configuration module
pub mod config;

use env_logger::{Builder as Logger, Env};
use futures::TryStreamExt;
use log::{error, info};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::MutexGuard;
use tokio::{runtime, sync::mpsc};
use crate::redis::{RedisClient, RedisContext};
use crate::config::{PULSAR_CONNECTION_DATA, WARP10_CONNECTION_DATA, REDIS_URL};

pub use job::{JobRessources, JobsHandler};
pub use pulsar_client::{PulsarClient, PulsarConnectionData};
pub use warp10::{Warp10Client, Warp10ConnectionData};



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

/// Helper to get DNS_RESOLVER env var as a [`SocketAddr`]
pub fn env_get_dns_resolver() -> SocketAddr {
    let env = std::env::var("DNS_RESOLVER").unwrap_or("127.0.0.1:53".to_owned());
    env.parse()
        .expect("Valid DNS_RESOLVER socket address expected")
}

/// Start logger with default log level : info (overridden by env var LOG_LEVEL)
pub fn init_logger() {
    let env = Env::new().filter_or("LOG_LEVEL", "info");
    Logger::from_env(env).init();
}

async fn use_redis(redis_url: &str) {
    let redis_client = RedisClient::new(redis_url);
    let redis_context = RedisContext::new("sample_key".to_string(), "sample_value".to_string());

    match redis_client.send_redis(&redis_context).await {
        Some(result) => println!("Redis send success: {}, at: {}", result.success, result.datetime),
        None => println!("Failed to send data to Redis"),
    }
}
/// Main async process : pulsar consumer loop
pub async fn main_process(
    pulsar_connection_data: PulsarConnectionData,
    warp10_connection_data: Warp10ConnectionData,
    task_pools_size: usize,
    redis_url: String,
) -> Option<()> {
    let ressources = JobRessources::default();
    let (warp10_snd, warp10_rcv): (mpsc::Sender<warp10::Data>, mpsc::Receiver<warp10::Data>) =
        mpsc::channel(512);
    let mut handler = JobsHandler::new(ressources, warp10_snd, task_pools_size);

    // Redis client initialization
    let redis_client = RedisClient::new(&redis_url);
    let redis_context = RedisContext::new("sample_key".to_string(), "sample_value".to_string());
    let redis_result = redis_client.send_redis(&redis_context).await;

    match redis_result {
        Some(result) => info!("Redis operation successful: {}, at: {}", result.success, result.datetime),
        None => error!("Redis operation failed"),
    }


    info!(
        "Connecting to pulsar topic {}...",
        pulsar_client::pulsar_link(&pulsar_connection_data)
    );
    let mut pulsar_client = match PulsarClient::new(pulsar_connection_data).await {
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
    let warp10_client = match Warp10Client::new(warp10_connection_data).await {
        Some(pc) => {
            info!("Connected to warp10 !");
            pc
        }
        None => {
            error!("Failed to connect to warp10");
            std::process::exit(1);
        }
    };

    tokio::task::spawn(warp10::warp10_sender(warp10_client, warp10_rcv, 10));

    while let Some(msg) = pulsar_client
        .consumer
        .try_next()
        .await
        .map_err(|e| {
            error!("Cant receive pulsar message : {e}");
            e
        })
        .ok()?
    {
        _ = pulsar_client.consumer.ack(&msg).await.map_err(|e| {
            error!("Can't acknoledge pulsar message : {e}");
            e
        });

        match msg.deserialize() {
            Ok(command) => {
                info!("Handling new pulsar command...");
                handler.handle_command(command);
            }
            Err(e) => {
                error!(
                    "Can't deserialize command [{}] : {e} ",
                    msg.payload
                        .data
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                ()
            }
        };
    }
    Some(())
}

/// The agent main entry point
pub fn main() {
    init_logger();

    let job_number = env_get_num("JOBS", 1);
    let task_pools_size = env_get_num("TASK_POOLS_SIZE", 128);

    // Access the global configuration data safely
    let pulsar_data: MutexGuard<PulsarConnectionData> = PULSAR_CONNECTION_DATA.lock().unwrap();
    let warp_data: MutexGuard<Warp10ConnectionData> = WARP10_CONNECTION_DATA.lock().unwrap();
    let redis_url = REDIS_URL.clone();

    // Clone the data out of the MutexGuard to use in async contexts
    let pulsar_connection_data = pulsar_data.clone();
    let warp_connection_data = warp_data.clone();

    // Build the Tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(job_number)
        .enable_all()
        .build()
        .expect("tokio runtime spawn");

    info!("Starting agent with {job_number} jobs...");
    runtime.block_on(main_process(
        pulsar_connection_data,
        warp_connection_data,
        task_pools_size,
        redis_url,
    ));
}