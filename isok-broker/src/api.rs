use crate::transport::{ResultTransport, TransportLayer};
use isok_data::broker_rpc::broker_server::{Broker, BrokerServer};
use isok_data::broker_rpc::{CheckBatchRequest, CheckBatchResponse, HealthRequest, HealthResponse};
use tonic::transport::Server;

pub(crate) struct BrokerGrpcService {
    transport_layer: TransportLayer,
}

#[tonic::async_trait]
impl Broker for BrokerGrpcService {
    #[tracing::instrument(skip(self, request), fields(agent_id = tracing::field::Empty, zone = tracing::field::Empty, region = tracing::field::Empty))]
    async fn batch_send(
        &self,
        mut request: tonic::Request<CheckBatchRequest>,
    ) -> Result<tonic::Response<CheckBatchResponse>, tonic::Status> {
        if let Some(tags) = &request.get_ref().tags {
            tracing::Span::current().record("agent_id", &tags.agent_id);
            tracing::Span::current().record("zone", &tags.zone);
            tracing::Span::current().record("region", &tags.region);
        } else {
            return Err(tonic::Status::invalid_argument("Missing tags"));
        }

        tracing::debug!(
            "Received a new batch of events, length: {}",
            request.get_ref().events.len()
        );

        let tags = request.get_ref().tags.clone();
        request.get_mut().events.iter_mut().for_each(|event| {
            event.tags = tags.clone();
        });

        // This piece of code implies that if one event of the batch fails,
        // the agent might duplicate whole batch to another broker. We should
        // treat a batch like a transaction.
        for event in request.get_ref().events.iter() {
            self.transport_layer
                .process_result(event)
                .await
                .map_err(|e| tonic::Status::internal(e.to_string()))?;
        }
        Ok(tonic::Response::new(CheckBatchResponse { error: None }))
    }
    async fn health(
        &self,
        _: tonic::Request<HealthRequest>,
    ) -> Result<tonic::Response<HealthResponse>, tonic::Status> {
        Ok(tonic::Response::new(HealthResponse { healthy: true }))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Unable to bind to address {0}")]
    ServerFailure(#[from] tonic::transport::Error),
}

impl BrokerGrpcService {
    pub fn new(transport_layer: TransportLayer) -> Self {
        Self { transport_layer }
    }

    pub async fn run_on(self, addr: std::net::SocketAddr) -> Result<(), ApiError> {
        tracing::info!("Starting API server on {}", addr);
        let server = BrokerServer::new(self);
        Server::builder()
            .add_service(server)
            .serve(addr)
            .await
            .map_err(ApiError::ServerFailure)
    }
}
