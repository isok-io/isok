mod api;
pub mod config;
mod db;
mod errors;
use crate::api::ApiStateInner;
use crate::config::Config;
use crate::db::DbHandler;
use errors::Result;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};

pub async fn run(config: Config) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    let mut graceful = true;

    let db_handler = DbHandler::connect(&config.database.database_url).await?;

    let mut tasks: JoinSet<Result<()>> = JoinSet::new();
    {
        let shutdown_rx = shutdown_rx;
        let db = db_handler.clone();
        tasks.spawn(async {
            api::run(config.api, Arc::new(ApiStateInner { _db: db }), shutdown_rx).await
        });
    }

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

                let db_handler = db_handler.clone();
                tasks.spawn(async move {db_handler.close().await});
            }
        }
    }

    tasks.shutdown().await;

    Ok(())
}
