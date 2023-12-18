pub mod http;
pub mod icmp;
pub mod job;
pub mod magic_pool;
pub mod pulsar_client;
pub mod tcp;
pub mod warp10;

use env_logger::{Builder as Logger, Env};
use futures::TryStreamExt;
use log::{error, info};
use std::str::FromStr;
use tokio::runtime;

pub use job::{JobRessources, JobsHandler};
pub use pulsar_client::{PulsarClient, PulsarConnectionData};
pub use tokio::sync::mpsc;
pub use warp10::{Warp10Client, Warp10ConnectionData};

fn env_panic(env: &'static str) -> impl Fn(std::env::VarError) -> String {
    move |e| {
        panic!("{env} is not set ({})", e);
    }
}

fn env_get(env: &'static str) -> String {
    std::env::var(env).unwrap_or_else(env_panic(env))
}

fn env_parse_panic<T: FromStr>(env: &'static str, val: String) -> impl Fn(T::Err) -> T {
    move |_| {
        panic!("can't parse {env} ({val})");
    }
}

fn env_get_num<T: FromStr>(env: &'static str, other: T) -> T {
    match std::env::var(env) {
        Ok(v) => v.parse::<T>().unwrap_or_else(env_parse_panic::<T>(env, v)),
        Err(_) => other,
    }
}

// fn get_dns_resolver() -> SocketAddr {
//     let env = std::env::var("DNS_RESOLVER");
// }

pub fn init_logger() {
    let env = Env::new().filter_or("LOG_LEVEL", "info");
    Logger::from_env(env).init();
}

async fn main_process(
    pulsar_connection_data: PulsarConnectionData,
    warp10_connection_data: Warp10ConnectionData,
    task_pools_size: usize,
) -> Option<()> {
    let ressources = JobRessources::default();
    let (warp10_snd, warp10_rcv): (mpsc::Sender<warp10::Data>, mpsc::Receiver<warp10::Data>) =
        mpsc::channel(512);
    let mut handler = JobsHandler::new(ressources, warp10_snd, task_pools_size);

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
                info!("handle new check");
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

fn main() {
    init_logger();

    let job_number = env_get_num("JOBS", 1);
    let task_pools_size = env_get_num("TASK_POOLS_SIZE", 128);

    let pulsar_address = env_get("PULSAR_ADDRESS");
    let pulsar_token = env_get("PULSAR_TOKEN");
    let pulsar_tenant = env_get("PULSAR_TENANT");
    let pulsar_namespace = env_get("PULSAR_NAMESPACE");
    let pulsar_topic = env_get("PULSAR_TOPIC");

    let pulsar_connection_data = PulsarConnectionData {
        pulsar_address,
        pulsar_token,
        pulsar_tenant,
        pulsar_namespace,
        pulsar_topic,
    };

    let warp10_address = env_get("WARP10_ADDRESS");
    let warp10_token = env_get("WARP10_TOKEN");

    let warp_connection_data = Warp10ConnectionData {
        warp10_address,
        warp10_token,
    };

    let runtime = runtime::Builder::new_multi_thread()
        .worker_threads(job_number)
        .enable_all()
        .build()
        .expect("tokio runtime spawn");

    info!("Starting agent with {job_number} jobs...");
    runtime.block_on(main_process(
        pulsar_connection_data,
        warp_connection_data,
        task_pools_size,
    ));
}
