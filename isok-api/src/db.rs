use std::time::Duration;

use serde_json;
use sqlx::postgres::types::PgInterval;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use isok_data::check::{Check, CheckInput, CheckKind};

use crate::api::errors::RequestError;

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

pub fn map_row_not_found(
    err: sqlx::Error,
    model: &'static str,
    value: impl ToString,
) -> RequestError {
    if let sqlx::Error::RowNotFound = err {
        RequestError::NotFound {
            model,
            value: value.to_string(),
        }
    } else {
        RequestError::Sqlx(err)
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

    pub async fn get_checks(&self) -> Result<Vec<Check>, RequestError> {
        sqlx::query!(
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NULL
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
        }).fetch_all(&self.pool).await.map_err(RequestError::Sqlx)
    }

    pub async fn get_check(&self, check_id: Uuid) -> Result<Check, RequestError> {
        sqlx::query!(
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NULL
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
        .fetch_one(&self.pool).await.map_err(|e| map_row_not_found(e, "check" , check_id))
    }

    pub async fn insert_check(&self, check: CheckInput) -> Result<Check, RequestError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
                INSERT INTO checks(owner_id, kind, max_latency, interval, region, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING check_id,owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
            "#,
            check.owner_id,
            serde_json::to_value(check.kind).unwrap(),
            duration_to_pg_interval(check.max_latency),
            duration_to_pg_interval(check.interval),
            check.region,
            now,
            now
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
        .fetch_one(&self.pool).await.map_err(RequestError::Sqlx)
    }

    pub async fn change_check_kind(
        &self,
        check_id: Uuid,
        check_kind: CheckKind,
        organization_id: Uuid,
    ) -> Result<(), RequestError> {
        sqlx::query!(
            r#"
UPDATE checks SET kind = $1, updated_at = $2 WHERE check_id = $3 AND deleted_at IS NULL AND owner_id = $4
        "#,
            serde_json::to_value(check_kind).unwrap(),
            Utc::now(),
            check_id,
            organization_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| map_row_not_found(e, "check", check_id))
    }

    pub async fn change_check_interval(
        &self,
        check_id: Uuid,
        interval: Duration,
        organization_id: Uuid,
    ) -> Result<(), RequestError> {
        sqlx::query!(
            r#"
UPDATE checks SET interval = $1, updated_at = $2 WHERE check_id = $3 AND deleted_at IS NULL AND owner_id = $4
        "#,
            duration_to_pg_interval(interval),
            Utc::now(),
            check_id,
            organization_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| map_row_not_found(e, "check", check_id))
    }

    pub async fn change_check_max_latency(
        &self,
        check_id: Uuid,
        max_latency: Duration,
        organization_id: Uuid,
    ) -> Result<(), RequestError> {
        sqlx::query!(
            r#"
UPDATE checks SET max_latency = $1, updated_at = $2 WHERE check_id = $3 AND deleted_at IS NULL AND owner_id = $4
        "#,
            duration_to_pg_interval(max_latency),
            Utc::now(),
            check_id,
            organization_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| map_row_not_found(e, "check", check_id))
    }

    pub async fn delete_check(
        &self,
        check_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Check, RequestError> {
        sqlx::query!(
            r#"
            UPDATE checks SET deleted_at = $1 WHERE check_id = $2 AND deleted_at IS NULL AND owner_id = $3
            RETURNING check_id,owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
        "#,
            Utc::now(),
            check_id,
            organization_id
        )
        .map(|row| Check {
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
        .fetch_one(&self.pool)
        .await
        .map_err(|e| map_row_not_found(e, "check", check_id))
    }
}
