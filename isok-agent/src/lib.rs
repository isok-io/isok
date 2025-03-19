use tokio::join;

mod batch_sender;
use self::batch_sender::BatchSender;

pub mod config;
use self::config::{Config, GetJobsRegistry};

pub mod errors;
use self::errors::{Error, Result};

pub mod jobs;

mod registry;

mod state;

pub async fn run(config: Config) -> Result<()> {
    let registry = config.get_jobs_registry()?;

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| Error::UnableToInstallDefaultCryptoProvider)?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut batch_sender = BatchSender::new(config.result_sender_adapter, rx)
        .await
        .map_err(Error::UnableToCreateBatchSender)?;
    join!(registry.execute(tx), batch_sender.run());

    Ok(())
}
