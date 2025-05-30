mod api;
mod job;
mod scheduler;
mod state;

use isok_data::messages;
use isok_data::models::{AgentInput, CheckResult};
use reqwest::Url;
use std::time::Duration;
use std::{collections::HashMap, net::IpAddr, sync::Arc};

use api::api;
use isok_data::messages::broker_client::BrokerClient;
use state::AgentState;
use tokio::{
    net::TcpListener,
    runtime,
    sync::{
        RwLock,
        mpsc::{UnboundedReceiver, unbounded_channel},
    },
};
use tracing::{error, info, warn};
use uuid::Uuid;

pub static mut AGENT_ID: Option<String> = None;
pub static mut ZONE: Option<Uuid> = None;

/// Get env var and parse it
pub fn env_get<T>(env: &'static str) -> Option<T>
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    if let Ok(value) = std::env::var(env) {
        match value.parse::<T>() {
            Ok(parsed_value) => Some(parsed_value),
            Err(err) => {
                error!("Unable to parse environment variable {env} : {err}");
                std::process::exit(1);
            }
        }
    } else {
        None
    }
}

pub fn env_get_mandatory<T>(env: &'static str) -> T
where
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    match env_get(env) {
        Some(e) => e,
        None => {
            error!("Please provide env variable : {env}");
            std::process::exit(1);
        }
    }
}

async fn offload(grpc_endpoint: String, mut rx: UnboundedReceiver<CheckResult>) {
    let Ok(client) = tonic::transport::Channel::from_shared(grpc_endpoint.clone()) else {
        error!("Invalid grpc endpoint: {grpc_endpoint}");
        std::process::exit(1);
    };
    let Ok(mut client) = BrokerClient::connect(client).await else {
        error!("Failed to connect to broker");
        std::process::exit(1);
    };

    loop {
        while let Some(r) = rx.recv().await {
            let req: messages::CheckResult = r.into();
            for i in 0..4 {
                match client.send(req.clone()).await {
                    Ok(_) => break,
                    Err(error) => {
                        if i == 3 {
                            error!(?error, "Failed to send result to the broker");
                            std::process::exit(1);
                        }
                        warn!(
                            ?error,
                            "Failed to send result to the broker, retrying in {} seconds",
                            1 + i
                        );
                        let mut itv = tokio::time::interval(Duration::from_secs(1 + i));
                        itv.tick().await;
                        itv.tick().await;
                    }
                }
            }
        }
    }
}

async fn register_agent(agent_id: String, zone: Uuid) {
    info!("Agent is registering");
    let api_url = match env_get::<Url>("API_URL") {
        Some(api_url) => api_url,
        None => {
            if let Ok(_) = std::env::var("NO_REGISTER") {
                warn!("Agent is not registered in the api, you are on your own...");
                return;
            } else {
                error!("Unable to register agent to api, variable API_URL is MISSING!");
                std::process::exit(1);
            }
        }
    };

    let api_token: String = env_get_mandatory("API_TOKEN");

    let endpoint = env_get_mandatory("AGENT_ENDPOINT");

    let client = reqwest::Client::new();
    match client
        .post(api_url)
        .header("Authorization", format!("Bearer {api_token}"))
        .json(&AgentInput {
            id: agent_id,
            zone,
            endpoint,
            // TODO : put a real token here
            token: "token".to_string(),
            // TODO : read tags somewhere
            tags: HashMap::with_capacity(0),
        })
        .send()
        .await
    {
        Ok(response) => match response.error_for_status() {
            Ok(_response) => {}
            Err(err) => {
                error!("Could not register agent : {err}");
                std::process::exit(1);
            }
        },
        Err(err) => {
            error!("Could not register agent : {err}");
            std::process::exit(1);
        }
    }
}

async fn main_process() {
    let agent_id: String = env_get_mandatory("AGENT_ID");
    let zone: Uuid = env_get_mandatory("AGENT_ZONE");
    unsafe {
        AGENT_ID = Some(agent_id.clone());
        ZONE = Some(zone.clone());
    }

    register_agent(agent_id, zone).await;
    info!("Agent has started!");

    let address = env_get("ADDRESS").unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let port = env_get("PORT").unwrap_or(8080u16);

    let grpc_endpoint: String = env_get_mandatory("BROKER_ADDRESS");

    let (snd, rx) = unbounded_channel();
    let api = api(Arc::new(RwLock::new(AgentState::new(snd))));
    let listener = match TcpListener::bind(std::net::SocketAddr::new(address, port)).await {
        Ok(l) => l,
        Err(err) => {
            error!("Unable to bind to {address}:{port} : {err}");
            std::process::exit(1);
        }
    };

    tokio::select! {
        _ = offload(grpc_endpoint, rx) => {},
        _ = axum::serve(listener, api) => {}
    }
}

/// Start logger
#[inline]
pub fn init_logger() {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_env_var("LOG_LEVEL")
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn main() {
    init_logger();

    let worker_threads: Option<usize> = env_get("WORKER_THREADS");

    let mut runtime = runtime::Builder::new_multi_thread();
    if let Some(worker_threads) = worker_threads {
        runtime.worker_threads(worker_threads);
    }

    let runtime = match runtime.enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => {
            error!("Unable to start tokio runtime : {err}");
            std::process::exit(1);
        }
    };

    runtime.block_on(main_process())
}
