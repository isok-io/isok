pub(crate) mod kafka;
pub(crate) mod warp10;

use crate::config::Transport;
use crate::transport::kafka::KafkaMessageBroker;
use crate::transport::warp10::Warp10MetricsTransport;
use isok_data::broker_rpc::CheckResult;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Unable to create producer: {0}")]
    UnableToCreateProducer(#[from] rdkafka::error::KafkaError),
    #[error("Failed to create base Warp10 Client: {0}")]
    UnableToCreateWarp10Client(#[from] reqwest::Error),
    #[error("Failed to start transport layer to prevent insecure usage: {0}")]
    InsecureParams(String),
    #[error("The transport layer isn't able to process any result")]
    ServiceUnhealthy,
    #[error("Transport layer received a fatal error: {0}")]
    BatchFatalError(String),
}

#[enum_dispatch::enum_dispatch(ResultTransport)]
pub enum TransportLayer {
    Kafka(KafkaMessageBroker),
    Warp10(Warp10MetricsTransport),
}

impl TransportLayer {
    pub fn try_new(config: Transport) -> Result<Self, TransportError> {
        match config {
            Transport::Kafka(config) => Ok(Self::Kafka(KafkaMessageBroker::try_new(config)?)),
            Transport::Warp10(config) => Ok(Self::Warp10(Warp10MetricsTransport::try_new(config)?)),
        }
    }
}

#[enum_dispatch::enum_dispatch]
pub trait ResultTransport {
    async fn process_batch(&self, batch: &[CheckResult]) -> Result<(), TransportError> {
        for message in batch {
            self.process_result(message).await?;
        }
        Ok(())
    }
    async fn process_result(&self, result: &CheckResult) -> Result<(), TransportError>;
    async fn health_check(&self) -> Result<(), TransportError>;
}
