mod api;
mod job;
mod scheduler;
mod state;

use std::{collections::HashMap, net::IpAddr, sync::Arc};

use isok_data::models::{AgentInput, CheckResult};
use reqwest::Url;

use api::api;
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

async fn offload(mut rx: UnboundedReceiver<CheckResult>) {
    loop {
        while let Some(_r) = rx.recv().await {
            // trust me, it works
        }
    }
}

async fn register_agent() {
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
    let id = env_get_mandatory("AGENT_ID");
    let zone = env_get_mandatory("AGENT_ZONE");
    let endpoint = env_get_mandatory("AGENT_ENDPOINT");

    let client = reqwest::Client::new();
    match client
        .post(api_url)
        .header("Authorization", format!("Bearer {api_token}"))
        .json(&AgentInput {
            id,
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
    register_agent().await;
    info!("Agent has started!");

    let address = env_get("ADDRESS").unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let port = env_get("PORT").unwrap_or(8080u16);

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
        _ = offload(rx) => {},
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
