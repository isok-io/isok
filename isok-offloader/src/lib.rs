use crate::config::Config;
use tracing::info;

pub mod config;
mod errors;

use errors::Result;

pub async fn run(config: Config) -> Result<()> {
    info!(?config);

    Ok(())
}
