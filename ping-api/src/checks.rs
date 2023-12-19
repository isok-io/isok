use std::sync::{Arc, Mutex};
use axum::response::{IntoResponse, Response};
use sqlx::{Error, PgPool};
use sqlx::postgres::types::PgInterval;
use sqlx::types::chrono::Utc;
use sqlx::types::Json;
use uuid::Uuid;
use ping_data::check::{Check, CheckKind, CheckOutput};

pub mod api {
    use crate::errors::{Error, SQLError};
    use axum::extract::{Path, State};
    use axum::Json;
    use ping_data::check::{Check, CheckOutput};
    use uuid::Uuid;
    use crate::checks::CheckRepository;
    use crate::errors::Error::{NotFound, RepositoryError};

    pub async fn list_checks(State(check_repository): State<CheckRepository>) -> Result<Json<Vec<CheckOutput>>, Error> {
        let listed = check_repository.find_all()
            .await
            .map_err(|e| RepositoryError(SQLError(e)))?;

        let outputs: Vec<CheckOutput> = listed.iter().cloned().map(|check| check.into()).collect();
        Ok(outputs.into())
    }

    pub async fn get_check(State(check_repository): State<CheckRepository>, Path(check_id): Path<Uuid>) -> Result<Json<CheckOutput>, Error> {
        let option_check = check_repository.find_by_id(check_id)
            .await
            .map_err(|e| { RepositoryError(SQLError(e)) })?;

        let check = option_check
            .ok_or(NotFound)?;

        let check_output: CheckOutput = check.into();
        Ok(check_output.into())
    }

    pub async fn create_check() {
        todo!()
    }

    pub async fn update_check() {
        todo!()
    }

    pub async fn delete_check() {
        todo!()
    }
}

#[derive(Clone)]
pub struct CheckRepository {
    pool: PgPool,
}

impl CheckRepository {
    pub fn new(pool: PgPool) -> Self {
        CheckRepository { pool }
    }

    pub async fn find_all(&self) -> Result<Vec<Check>, Error> {
        sqlx::query_as!(
            Check,
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NOT NULL
            "#
        ).fetch_all(&self.pool).await
    }

    pub async fn find_by_id(&self, check_id: Uuid) -> Result<Option<Check>, Error> {
        sqlx::query_as!(
            Check,
            r#"
                SELECT check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at
                FROM checks
                WHERE deleted_at IS NOT NULL
                AND check_id = $1
            "#,
            check_id
        ).fetch_optional(&self.pool).await
    }

    pub async fn insert(&self, check: Check) -> Result<(), Error> {
        let Check {
            check_id,
            owner_id,
            kind,
            max_latency,
            interval,
            region,
            created_at,
            updated_at,
            deleted_at
        } = check;
        sqlx::query!(
            r#"
                INSERT INTO checks(check_id, owner_id, kind, max_latency, interval, region, created_at, updated_at, deleted_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            check_id,
            owner_id,
            Json(kind) as _,
            max_latency,
            interval,
            region,
            created_at,
            updated_at,
            deleted_at
        ).execute(&self.pool).await?;
        Ok(())
    }
}