use log::error;
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use ping_data::owner::{User, UserInput};

use crate::api::auth::UserPassword;
use crate::api::errors::DbQueryError;

#[derive(Clone, Debug)]
pub struct DbHandler {
    pool: PgPool,
}

impl DbHandler {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(uri: String) -> Option<Self> {
        match PgPoolOptions::new().connect(&uri).await {
            Ok(p) => Some(Self::new(p)),
            Err(e) => {
                error!("Error while connecting to the db: {e}");
                None
            }
        }
    }

    pub async fn get_user_password(
        &self,
        email_address: String,
    ) -> Result<UserPassword, DbQueryError> {
        sqlx::query_as!(
            UserPassword,
            r#"
                SELECT user_id, password FROM users WHERE email_address = $1 AND deleted_at IS NULL
            "#,
            email_address
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbQueryError)
    }

    pub async fn get_users(&self) -> Result<Vec<User>, DbQueryError> {
        sqlx::query_as!(
            User,
            r#"
                SELECT user_id as owner_id, username, password, email_address, created_at, updated_at, deleted_at FROM users WHERE deleted_at IS NULL
            "#
        )
            .fetch_all(&self.pool)
            .await.map_err(DbQueryError)
    }

    pub async fn get_user(&self, user_id: Uuid) -> Result<User, DbQueryError> {
        sqlx::query_as!(
            User,
            r#"
                SELECT user_id as owner_id, username, password, email_address, created_at, updated_at, deleted_at FROM users WHERE user_id = $1 AND deleted_at IS NULL
            "#, user_id
        )
            .fetch_one(&self.pool)
            .await.map_err(DbQueryError)
    }

    pub async fn insert_user(&self, user: UserInput) -> Result<(), DbQueryError> {
        let now = Utc::now();

        sqlx::query!(
            r#"
INSERT INTO users (username, password, email_address, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5)
        "#,
            user.username,
            user.password,
            user.email_address,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn update_user_username(
        &self,
        user_id: Uuid,
        username: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET username = $1, updated_at = $2 WHERE user_id = $3 AND deleted_at IS NULL
        "#,
            username,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn update_user_email(
        &self,
        user_id: Uuid,
        email_address: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET email_address = $1, updated_at = $2 WHERE user_id = $3 AND deleted_at IS NULL
        "#,
            email_address,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn update_user_password(
        &self,
        user_id: Uuid,
        password: String,
    ) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET password = $1, updated_at = $2 WHERE user_id = $3
        "#,
            password,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }

    pub async fn delete_user(&self, user_id: Uuid) -> Result<(), DbQueryError> {
        sqlx::query!(
            r#"
UPDATE users SET deleted_at = $1 WHERE user_id = $2
        "#,
            Utc::now(),
            user_id
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(DbQueryError)
    }
}
