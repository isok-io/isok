mod api;
pub mod config;
mod db;
mod errors;
mod services;

use crate::api::{ApiStateInner, Hasher};
use crate::config::Config;
use crate::db::DbHandler;
use crate::services::agents::AgentsHandler;
use crate::services::warp10::Warp10;
use biscuit_auth::KeyPair;
use errors::Result;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{error, info};

pub async fn run(config: Config) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    let mut graceful = true;

    let db_handler = DbHandler::connect(&config.database.database_url).await?;

    let mut tasks: JoinSet<Result<()>> = JoinSet::new();
    let agents_handler = AgentsHandler::new(
        config.agents_handler,
        db_handler.clone(),
        &mut tasks,
        shutdown_rx.clone(),
    );
    {
        let shutdown_rx = shutdown_rx;
        let hasher = Hasher::new(&config.api.argon2_params);
        let keypair = KeyPair::from(&config.api.private_key);
        let db = db_handler.clone();
        let warp10 = Warp10::new(config.warp10);
        tasks.spawn(async {
            api::run(
                config.api,
                Arc::new(ApiStateInner {
                    db,
                    agents: agents_handler,
                    hasher,
                    keypair,
                    warp10,
                }),
                shutdown_rx,
            )
            .await
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
