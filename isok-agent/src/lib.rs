use tokio::join;

use crate::batch_sender::BatchSender;
use crate::config::{Config, GetJobsRegistry};
use crate::errors::{Error, Result};

mod batch_sender;
pub mod config;
pub mod errors;
pub mod jobs;
mod registry;
mod state;

pub async fn run(config: Config) -> Result<()> {
    let registry = config.get_jobs_registry()?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut batch_sender = BatchSender::new(config.result_sender_adapter, rx)
        .await
        .map_err(|e| Error::UnableToCreateBatchSender(e))?;
    join!(registry.execute(tx), batch_sender.run());

    Ok(())
}
