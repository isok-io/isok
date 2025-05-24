mod agents;
mod checks;
mod organisations;
mod regions;
mod tenants;
mod users;

use crate::Result;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

#[derive(Clone, Debug)]
pub struct DbHandler {
    pool: PgPool,
}

impl DbHandler {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(uri: &str) -> Result<Self> {
        PgPoolOptions::new()
            .connect(uri)
            .await
            .map(Self::new)
            .map_err(Into::into)
    }

    pub async fn close(&self) -> Result<()> {
        info!("Shutting down db pool");

        self.pool.close().await;
        Ok(())
    }
}
