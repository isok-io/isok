use ping_data::check::{Check, CheckInput};
use serde_json;
use sqlx::postgres::types::PgInterval;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::Utc;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::api::errors::DbQueryError;

pub fn pg_interval_to_duration(interval: PgInterval) -> Duration {
    Duration::from_micros(interval.microseconds as u64)
}

pub fn duration_to_pg_interval(duration: Duration) -> PgInterval {
    PgInterval {
        months: 0,
        days: 0,
        microseconds: duration.as_micros() as i64,
    }
}

#[derive(Clone, Debug)]
pub struct DbHandler {
    pool: PgPool,
}

impl DbHandler {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(uri: &str) -> Option<Self> {
        PgPoolOptions::new()
            .connect(uri)
            .await
            .map(|p| Self::new(p))
            .ok()
    }

    pub async fn get_checks(&self) -> Result<Vec<Check>, DbQueryError> {
        sqlx::query!(
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NOT NULL
            "#
        ).map(|row| Check {
            check_id: row.check_id,
            owner_id: row.owner_id,
            kind: serde_json::from_value(row.kind).unwrap(),
            max_latency: pg_interval_to_duration(row.max_latency),
            interval: pg_interval_to_duration(row.interval),
            region: row.region,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }).fetch_all(&self.pool).await.map_err(|e| DbQueryError(e))
    }

    pub async fn get_check(&self, check_id: Uuid) -> Result<Check, DbQueryError> {
        sqlx::query!(
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NOT NULL
                AND check_id = $1
            "#,
            check_id
        ).map(|row| Check {
            check_id: row.check_id,
            owner_id: row.owner_id,
            kind: serde_json::from_value(row.kind).unwrap(),
            max_latency: pg_interval_to_duration(row.max_latency),
            interval: pg_interval_to_duration(row.interval),
            region: row.region,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        })
        .fetch_one(&self.pool).await.map_err(|e| DbQueryError(e))
    }

    pub async fn insert_check(&self, check: CheckInput) -> Result<(), DbQueryError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
                INSERT INTO checks(owner_id, kind, max_latency, interval, region, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            check.owner_id,
            serde_json::to_value(check.kind).unwrap(),
            duration_to_pg_interval(check.max_latency),
            duration_to_pg_interval(check.interval),
            check.region,
            now,
            now
        ).execute(&self.pool).await.map(|_| ()).map_err(|e| DbQueryError(e))
    }
}
