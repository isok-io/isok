use std::net::SocketAddr;
use std::str::FromStr;

use env_logger::{Builder as Logger, Env};
use futures::TryStreamExt;
use log::{error, info};
use tokio::{runtime, sync::mpsc};

pub use job::{JobResources, JobsHandler};
use ping_data::pulsar_messages::{CheckMessage, CheckResult, CheckType};
pub use pulsar_client::{PulsarClient, PulsarConnectionData};

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
mod pulsar_source;

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

/// Main async process : pulsar consumer loop
pub async fn main_process(
    pulsar_connection_data: PulsarConnectionData,
    task_pools_size: usize,
    agent_id: String
) -> Option<()> {
    let resources = JobResources::default();
    let (pulsar_sender, pulsar_receiver): (mpsc::Sender<CheckMessage>, mpsc::Receiver<CheckMessage>) =
        mpsc::channel(512);
    let mut handler = JobsHandler::new(resources, pulsar_sender, task_pools_size);

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

    let http_producer = match pulsar_client.create_producer(CheckType::Http).await {
        Some(producer) => {
            info!("Connected to pulsar topic !");
            producer
        }
        None => {
            error!("Failed to connect to pulsar topic");
            std::process::exit(1);
        }
    };

    tokio::task::spawn(pulsar_source::pulsar_sink(http_producer, pulsar_receiver));

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

    let pulsar_address = env_get("PULSAR_ADDRESS");
    let pulsar_token = env_get("PULSAR_TOKEN");
    let pulsar_tenant = env_get("PULSAR_TENANT");
    let pulsar_namespace = env_get("PULSAR_NAMESPACE");
    let pulsar_topic = env_get("PULSAR_TOPIC");

    let agent_id = env_get("AGENT_ID");

    let pulsar_connection_data = PulsarConnectionData {
        pulsar_address,
        pulsar_token,
        pulsar_tenant,
        pulsar_namespace,
        pulsar_consumer_topic: pulsar_topic,
    };

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(job_number)
        .enable_all()
        .build()
        .expect("tokio runtime spawn");

    info!("Starting agent with {job_number} jobs...");
    runtime.block_on(main_process(
        pulsar_connection_data,
        task_pools_size,
        agent_id
    ));
}
