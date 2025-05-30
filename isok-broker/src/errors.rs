use rdkafka::error::KafkaError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("kafka error: {0}")]
    Kafka(#[from] KafkaError),
    #[error("Unable to bind to address {0}")]
    ServerFailure(#[from] tonic::transport::Error),
    #[error("Encode error: {0}")]
    Encode(String),
}

pub type Result<T> = std::result::Result<T, Error>;
