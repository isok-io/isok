pub use std::path::PathBuf;
pub use std::sync::Arc;

pub use argon2::Params;
pub use biscuit_auth::PrivateKey;
#[cfg(not(feature = "env_config"))]
pub use clap::Parser;
pub use env_logger::{Builder as Logger, Env};
pub use log::{debug, error, info};

pub use crate::api::{routes, ServerState};
pub use crate::config::IncompleteConfig;
pub use crate::db::DbHandler;

pub mod api;
pub mod config;
pub mod db;
pub mod utils;

/// Get env var as string or panic
pub fn env_get(env: &'static str) -> String {
    let env_panic = |e| {
        panic!("{env} is not set ({})", e);
    };

    std::env::var(env).map_err(env_panic).unwrap()
}

/// Start logger with default log level : info (overridden by env var LOG_LEVEL)
pub fn init_logger() {
    let env = Env::new().filter_or("LOG_LEVEL", "info");
    Logger::from_env(env).init();
}

#[cfg(not(feature = "env_config"))]
#[derive(Parser, Debug)]
pub struct Cli {
    /// Set config toml file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Set listening address
    #[arg(short, long)]
    address: Option<String>,

    /// Set listening port
    #[arg(short, long)]
    port: Option<u16>,

    /// Set database uri
    #[arg(short, long)]
    db: Option<String>,

    /// Set auth token private key
    #[arg(short, long)]
    token: Option<String>,

    /// Set api connection
    #[arg(long)]
    api: Option<Vec<String>>,

    /// Set password hash memory size in KiB blocks
    #[arg(long)]
    hash_m_cost: Option<u32>,

    /// Set password hash iterations
    #[arg(long)]
    hash_t_cost: Option<u32>,

    /// Set password hash memory parallelism degree
    #[arg(long)]
    hash_p_cost: Option<u32>,

    /// Set password hash length
    #[arg(long)]
    hash_len: Option<usize>,
}

#[tokio::main]
async fn main() {
    init_logger();

    #[cfg(not(feature = "env_config"))]
    let config = {
        let cli = Cli::parse();
        debug!("{:#?}", cli);
        match cli.config {
            Some(ref path) => IncompleteConfig::from_file(path)
                .map_err(|e| {
                    error!("Failed to open config file: {e}");
                    std::process::exit(1)
                })
                .unwrap()
                .map_err(|e| {
                    error!("Failed to parse config file: {e}");
                    std::process::exit(1)
                })
                .unwrap()
                .merge(IncompleteConfig::from_env()),
            None => IncompleteConfig::from_env(),
        }
        .merge(IncompleteConfig::from_cli(cli))
        .to_config()
    };

    #[cfg(feature = "env_config")]
    let config = IncompleteConfig::from_env().to_config();

    info!(
        "Starting proxy server at {}:{}...",
        config.address.clone(),
        config.port
    );
    let app = routes::app(ServerState {
        private_key: Arc::new(
            PrivateKey::from_bytes_hex(config.token.as_str())
                .map_err(|e| {
                    error!("Failed to parse token: {e}");
                    std::process::exit(1)
                })
                .unwrap(),
        ),
        argon2_params: Arc::new(
            Params::new(
                config.hash_m_cost,
                config.hash_t_cost,
                config.hash_p_cost,
                config.hash_len,
            )
            .map_err(|e| {
                error!("Unable to init argon2 parameters: {e}");
                std::process::exit(1);
            })
            .unwrap(),
        ),
        db: Arc::new(DbHandler::connect(config.db).await.unwrap()),
        apis: Arc::new(
            config
                .apis
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned().uri.into()))
                .collect(),
        ),
    });

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.address, config.port,))
        .await
        .expect("Unable to bind");

    axum::serve(listener, app).await.unwrap()
}
