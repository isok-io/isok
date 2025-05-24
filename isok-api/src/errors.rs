use axum::http::header::InvalidHeaderValue;
use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum Error {
    #[error("listener initialization failed: {0}")]
    ApiInitialization(#[from] io::Error),
    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("argon2 error: {0}")]
    PasswordHash(#[from] argon2::password_hash::Error),
    #[error("wrong credentials")]
    WrongCredentials,
    #[error("biscuit error: {0}")]
    Biscuit(#[from] biscuit_auth::error::Token),
    #[error("failed to parse cors origins: {0}")]
    CorsOrigins(#[from] InvalidHeaderValue),
    #[error("request error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
