use clap::Parser;
use isok_api::config::Config;
use isok_api::run;
use std::path::PathBuf;
use tracing::{debug, error, info};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    #[arg(short, long = "config", env = "ISOK_API_CONFIG_PATH")]
    config_path: Option<PathBuf>,
}

impl CliArgs {
    fn get_possible_paths(bin_name: &str) -> Vec<PathBuf> {
        vec![
            PathBuf::from(format!("/etc/{bin_name}/api.toml")),
            PathBuf::from(format!("./{bin_name}/api.toml")),
            PathBuf::from("./api.toml"),
        ]
    }
}

/// Init tracing
#[inline]
pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_env_var("LOG_LEVEL")
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let mut cli = CliArgs::parse();

    if cli.config_path.is_none() {
        let possible_paths = CliArgs::get_possible_paths(env!("CARGO_PKG_NAME"));
        debug!(
            "No config file provided, looking for one in {:?}",
            possible_paths
        );
        for path in possible_paths {
            if path.exists() {
                info!("Using config file at {}", path.display());
                cli.config_path = Some(path);
                break;
            }
        }
    }

    let config = cli
        .config_path
        .map(|path| {
            Config::from_file(path).unwrap_or_else(|error| {
                error!(?error, "Failed to load config file");
                std::process::exit(1);
            })
        })
        .unwrap_or_default();

    if let Err(error) = run(config).await {
        error!(?error, "Api crashed");
        std::process::exit(1);
    }
}
