use crate::config::{BrokerConfig, ResultSenderAdapter, SocketConfig};
use enum_dispatch::enum_dispatch;
use isok_data::broker_rpc::broker_client::BrokerClient;
use isok_data::broker_rpc::check_result::Details;
use isok_data::broker_rpc::{BrokerGrpcClient, CheckJobMetrics, CheckJobStatus, CheckResult, Tags};
use isok_data::JobId;
use prost::Message;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant;
use tonic::transport::Channel;

#[derive(Debug)]
pub struct JobResult {
    pub id: JobId,
    pub name: String,
    pub run_at: Instant,
    pub status: CheckJobStatus,
    pub latency: Option<Duration>,
    pub details: Option<Details>,
}

impl JobResult {
    pub fn new(id: JobId, name: String) -> Self {
        JobResult {
            id,
            name,
            run_at: Instant::now(),
            status: CheckJobStatus::Unknown,
            details: None,
            latency: None,
        }
    }

    pub(crate) fn set_status(&mut self, status: CheckJobStatus) {
        self.status = status;
    }

    pub(crate) fn set_latency(&mut self, latency: Duration) {
        self.latency = Some(latency);
    }

    pub(crate) fn set_details(&mut self, details: Option<Details>) {
        self.details = details;
    }
}

pub struct BatchSender {
    connector: BatchSenderType,
    rx: UnboundedReceiver<JobResult>,
}

impl BatchSender {
    pub async fn new(
        adapter_cfg: ResultSenderAdapter,
        rx: UnboundedReceiver<JobResult>,
    ) -> Result<Self, BatchSenderError> {
        let connector = match adapter_cfg {
            ResultSenderAdapter::Stdout => BatchSenderType::Stdout(StdoutBatchSender::new()),
            ResultSenderAdapter::Broker(config) => {
                BatchSenderType::Broker(BrokerBatchSender::new(config).await?)
            }
            ResultSenderAdapter::Socket(config) => {
                BatchSenderType::Socket(SocketBatchSender::new(config).await?)
            }
        };
        Ok(BatchSender { rx, connector })
    }

    pub async fn run(&mut self) {
        while let Some(job) = self.rx.recv().await {
            match self.connector.send(job).await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Unable to send job result {:?}", e);
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BatchSenderError {
    #[error("Provided Broker URL is either invalid or unreachable")]
    InvalidBrokerEndpointConfiguration,
    #[error("Broker is not healthy")]
    BrokerUnhealthy,
    #[error("Unable to send job result: {0}")]
    UnableToSendBatch(String),
    #[error("Unable to forward job result to socket: {0}")]
    WriteSocketError(String),
    #[error("Unable to connect to socket: {0}")]
    OpenSocketError(String),
}

#[enum_dispatch(BatchSenderOutput)]
pub enum BatchSenderType {
    Stdout(StdoutBatchSender),
    Broker(BrokerBatchSender),
    Socket(SocketBatchSender),
}

#[enum_dispatch]
pub trait BatchSenderOutput {
    async fn send(&mut self, job_result: JobResult) -> Result<(), BatchSenderError>;

    async fn health_check(&mut self) -> Result<(), BatchSenderError>;
}

pub struct SocketBatchSender {
    stream: UnixStream,
}

impl SocketBatchSender {
    pub async fn new(config: SocketConfig) -> Result<Self, BatchSenderError> {
        let stream = UnixStream::connect(config.path)
            .await
            .map_err(|e| BatchSenderError::OpenSocketError(e.to_string()))?;
        Ok(Self { stream })
    }
}

impl BatchSenderOutput for SocketBatchSender {
    async fn send(&mut self, job_result: JobResult) -> Result<(), BatchSenderError> {
        let request = isok_data::broker_rpc::CheckBatchRequest {
            created_at: None,
            tags: Some(Tags {
                agent_id: "local-agent".to_string(),
                zone: "dev".to_string(),
                region: "localhost".to_string(),
            }),
            events: vec![CheckResult {
                id_ulid: job_result.id.clone().to_string(),
                run_at: None,
                status: job_result.status.into(),
                metrics: Some(CheckJobMetrics {
                    latency: job_result.latency.map(|d| d.as_millis() as u64),
                }),
                tags: None,
                details: job_result.details,
                pretty_name: Some(job_result.name),
            }],
        };
        let mut buffer = Vec::new();
        request.encode(&mut buffer).unwrap();

        // We first write the length of the message into 8 bytes (more than necessary),
        // then the message itself
        let len = buffer.len();
        // Linter rings a bell, but we need to define the array size
        // to avoid variable array size
        #[allow(unused_assignments)]
        let mut len_bytes = [0u8; 8];
        len_bytes = len.to_be_bytes();
        self.stream
            .write_all(&len_bytes)
            .await
            .map_err(|e| BatchSenderError::WriteSocketError(e.to_string()))?;

        self.stream
            .write_all(&buffer)
            .await
            .map_err(|e| BatchSenderError::WriteSocketError(e.to_string()))?;
        Ok(())
    }

    async fn health_check(&mut self) -> Result<(), BatchSenderError> {
        self.stream
            .writable()
            .await
            .map_err(|e| BatchSenderError::WriteSocketError(e.to_string()))
    }
}

pub struct StdoutBatchSender {}

impl StdoutBatchSender {
    pub fn new() -> Self {
        Self {}
    }
}

impl BatchSenderOutput for StdoutBatchSender {
    async fn send(&mut self, job_result: JobResult) -> Result<(), BatchSenderError> {
        tracing::info!(job_id = ?job_result.id, job_status = ?job_result.status, "Job result");
        Ok(())
    }

    async fn health_check(&mut self) -> Result<(), BatchSenderError> {
        Ok(())
    }
}

pub struct BrokerBatchSender {
    client: BrokerGrpcClient,
    zone: String,
    region: String,
    agent_id: String,

    backlog: Vec<JobResult>,

    batch: u64,
    batch_interval: u64,

    last_batch: Instant,
}

impl BrokerBatchSender {
    const MAX_RETRY_COUNT: u8 = 3;
    const DELAY_BETWEEN_RETRIES: Duration = Duration::from_secs(2);
    pub async fn new(config: BrokerConfig) -> Result<Self, BatchSenderError> {
        let client = BrokerClient::connect(config.main_broker.clone())
            .await
            .map_err(|_| BatchSenderError::InvalidBrokerEndpointConfiguration)?;
        Self::new_with_client(config, client).await
    }

    pub async fn new_with_client(
        config: BrokerConfig,
        client: BrokerClient<Channel>,
    ) -> Result<Self, BatchSenderError> {
        if config.batch_interval == 0 {
            tracing::warn!("Batch interval is set to 0, batch will be sent immediately");
        }

        let batch = config.batch.max(1);

        Ok(BrokerBatchSender {
            client,
            backlog: Vec::with_capacity(batch as usize),
            agent_id: config.agent_id,
            zone: config.zone,
            region: config.region,
            batch: batch as u64,
            batch_interval: config.batch_interval,
            last_batch: Instant::now(),
        })
    }

    async fn drain_and_send(&mut self) -> Result<(), BatchSenderError> {
        if self.backlog.is_empty() {
            return Ok(());
        }
        let events = self.backlog.drain(..).map(|e| e.into()).collect();
        self.send_batch(events, None).await?;
        self.last_batch = Instant::now();

        Ok(())
    }

    async fn send_batch(
        &mut self,
        events: Vec<CheckResult>,
        retry_count: Option<u8>,
    ) -> Result<(), BatchSenderError> {
        let retry_count = retry_count.unwrap_or(0);
        let batch_request = isok_data::broker_rpc::CheckBatchRequest {
            created_at: None,
            tags: Some(Tags {
                agent_id: self.agent_id.clone(),
                zone: self.zone.clone(),
                region: self.region.clone(),
            }),
            events: events.clone(),
        };

        match self.client.batch_send(batch_request).await {
            Ok(_) => Ok(()),
            Err(e) if retry_count < Self::MAX_RETRY_COUNT => {
                tracing::warn!(code = ?e.code(), message = ?e.message(), "Batch send failed, retrying");
                tokio::time::sleep(Self::DELAY_BETWEEN_RETRIES).await;
                Box::pin(self.send_batch(events, Some(retry_count + 1))).await
            }
            Err(e) => Err(BatchSenderError::UnableToSendBatch(e.to_string())),
        }
    }
}

impl From<JobResult> for CheckResult {
    fn from(value: JobResult) -> Self {
        Self {
            id_ulid: value.id.to_string(),
            run_at: None,
            status: value.status.into(),
            metrics: Some(CheckJobMetrics {
                latency: value.latency.map(|d| d.as_millis() as u64),
            }),
            tags: None,
            details: value.details,
            pretty_name: Some(value.name),
        }
    }
}

impl BatchSenderOutput for BrokerBatchSender {
    async fn send(&mut self, job_result: JobResult) -> Result<(), BatchSenderError> {
        self.backlog.push(job_result);

        // Won't scale for high volumes of messages
        if self.backlog.len() == self.batch as usize {
            tracing::debug!(
                batch_size = self.backlog.len(),
                "Batch is full, draining and sending"
            );
            self.drain_and_send().await?;
            return Ok(());
        }

        if self.last_batch.elapsed() > Duration::from_secs(self.batch_interval) {
            self.drain_and_send().await?;
        }

        Ok(())
    }

    async fn health_check(&mut self) -> Result<(), BatchSenderError> {
        match self
            .client
            .health(isok_data::broker_rpc::HealthRequest {})
            .await
        {
            Ok(response) => {
                if !response.get_ref().healthy {
                    return Err(BatchSenderError::BrokerUnhealthy);
                }
                Ok(())
            }
            Err(_) => Err(BatchSenderError::BrokerUnhealthy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hyper_util::rt::TokioIo;
    use isok_data::broker_rpc::broker_server::{Broker, BrokerServer};
    use isok_data::broker_rpc::{
        CheckBatchRequest, CheckBatchResponse, HealthRequest, HealthResponse,
    };
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tokio::io::DuplexStream;
    use tonic::codegen::tokio_stream;
    use tonic::transport::{Endpoint, Server, Uri};
    use tower::service_fn;

    #[derive(Default)]
    struct DummyBroker {
        batch_send_response: CheckBatchResponse,
        health_response: HealthResponse,
    }

    impl DummyBroker {
        fn spawn_server(self, server: DuplexStream) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                let broker = Server::builder()
                    .add_service(BrokerServer::new(self))
                    .serve_with_incoming(tokio_stream::once(Ok::<_, std::io::Error>(server)));
                broker.await.unwrap();
            })
        }

        async fn create_client(client: DuplexStream) -> BrokerClient<Channel> {
            let mut client = Some(client);
            let channel = Endpoint::try_from("http://[::]:50051")
                .unwrap()
                .connect_with_connector(service_fn(move |_: Uri| {
                    let client = client.take();

                    async move {
                        if let Some(client) = client {
                            Ok(TokioIo::new(client))
                        } else {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Client already taken",
                            ))
                        }
                    }
                }))
                .await
                .unwrap();
            BrokerClient::new(channel)
        }
    }

    #[async_trait]
    impl Broker for DummyBroker {
        async fn batch_send(
            &self,
            _: tonic::Request<CheckBatchRequest>,
        ) -> Result<tonic::Response<CheckBatchResponse>, tonic::Status> {
            Ok(tonic::Response::new(self.batch_send_response.clone()))
        }

        async fn health(
            &self,
            _: tonic::Request<HealthRequest>,
        ) -> Result<tonic::Response<HealthResponse>, tonic::Status> {
            Ok(tonic::Response::new(self.health_response.clone()))
        }
    }

    fn create_broker_config(batch: u64, batch_interval: u64) -> BrokerConfig {
        BrokerConfig {
            main_broker: "127.0.0.1:50551".to_string(),
            fallback_brokers: vec![],
            agent_id: "test".to_string(),
            zone: "dev".to_string(),
            region: "localhost".to_string(),
            batch,
            batch_interval,
        }
    }

    // Ensure the batch is sent when the batch interval is passed
    #[tokio::test]
    async fn test_batch_sender() {
        let (client, server) = tokio::io::duplex(1024);
        let _ = DummyBroker::default().spawn_server(server);
        let client = DummyBroker::create_client(client).await;
        let config = create_broker_config(10, 5);
        let mut sender = BrokerBatchSender::new_with_client(config, client)
            .await
            .expect("Expected to create batch sender");

        let job_result = JobResult::new(JobId::generate(), "test".to_string());
        let snapshot_last_batch = sender.last_batch.clone();
        sender.send(job_result).await.unwrap();
        assert_eq!(sender.backlog.len(), 1);

        // Wait 5 seconds to make the batch interval pass
        tokio::time::sleep(Duration::from_secs(5)).await;

        sender.drain_and_send().await.unwrap();
        assert_eq!(sender.backlog.len(), 0);
        assert!(sender.last_batch > snapshot_last_batch);
    }

    // Expect any message to be sent immediately, when the
    // batch size is set to 0
    #[tokio::test]
    async fn test_batch_sender_with_zero_batch() {
        let (client, server) = tokio::io::duplex(1024);
        let _ = DummyBroker::default().spawn_server(server);
        let client = DummyBroker::create_client(client).await;
        let config = create_broker_config(0, 5);
        let mut sender = BrokerBatchSender::new_with_client(config, client)
            .await
            .expect("Expected to create batch sender");

        let job_result = JobResult::new(JobId::generate(), "test".to_string());
        let snapshot_last_batch = sender.last_batch.clone();
        sender.send(job_result).await.unwrap();
        assert_eq!(sender.backlog.len(), 0);
        // We should have sent the batch
        assert!(sender.last_batch > snapshot_last_batch);
    }

    // Expect any message to be sent immediately, when the
    // batch interval is set to 0
    #[tokio::test]
    async fn test_batch_sender_with_zero_interval() {
        let (client, server) = tokio::io::duplex(1024);
        let _ = DummyBroker::default().spawn_server(server);
        let client = DummyBroker::create_client(client).await;
        let config = create_broker_config(10, 0);
        let mut sender = BrokerBatchSender::new_with_client(config, client)
            .await
            .expect("Expected to create batch sender");

        let job_result = JobResult::new(JobId::generate(), "test".to_string());
        let snapshot_last_batch = sender.last_batch.clone();
        sender.send(job_result).await.unwrap();
        assert_eq!(sender.backlog.len(), 0);
        // We should have sent the batch
        assert!(sender.last_batch > snapshot_last_batch);
    }
}
