use crate::config::{Config, Exporter};
use tokio::task::JoinSet;
use tracing::{error, info};

pub mod config;
mod errors;
mod exporter;
mod kafka;

use crate::exporter::stdout::StdoutExporter;
use crate::exporter::warp10::Warp10Exporter;
use crate::kafka::Kafka;
use errors::Result;

pub async fn run(config: Config) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    let mut graceful = true;
    let mut tasks: JoinSet<Result<()>> = JoinSet::new();

    let exporter: Box<dyn exporter::Exporter + Send> = match config.exporter {
        Exporter::Warp10(config) => {
            Box::new(Warp10Exporter::new(config, &mut tasks, shutdown_rx.clone()))
        }
        Exporter::Stdout => Box::new(StdoutExporter::new()),
    };

    Kafka::new(config.kafka, exporter, &mut tasks, shutdown_rx).await?;

    loop {
        tokio::select! {
            Some(res) = tasks.join_next() => {
                match res {
                    Ok(Ok(_)) => (),
                    Ok(Err(error)) => {
                        error!(?error, "Task finished with error");
                        return Err(error);
                    },
                    Err(error) => {
                        error!(?error, "Error joining task");
                        return Err(error.into())
                    }
                };

                if tasks.is_empty() {
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if !graceful {
                    break;
                }
                info!("Graceful shutdown");
                graceful = false;
                _ = shutdown_tx.send(());
            }
        }
    }

    tasks.shutdown().await;

    Ok(())
}
