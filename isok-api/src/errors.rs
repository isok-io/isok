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
}

pub type Result<T> = std::result::Result<T, Error>;
