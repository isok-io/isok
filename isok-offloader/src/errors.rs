use rdkafka::error::KafkaError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("kafka error: {0}")]
    Kafka(#[from] KafkaError),
    #[error("warp10 error")]
    Warp10,
}

pub type Result<T> = std::result::Result<T, Error>;
