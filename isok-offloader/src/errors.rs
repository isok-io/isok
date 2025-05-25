use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;
