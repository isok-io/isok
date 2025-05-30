use crate::Result;
use crate::errors::Error;
use crate::kafka::Kafka;
use isok_data::messages::CheckResult;
use isok_data::messages::broker_server::{Broker, BrokerServer};
use tonic::transport::Server;
use tracing::info;

pub(crate) struct BrokerGrpcService {
    kafka: Kafka,
}

#[tonic::async_trait]
impl Broker for BrokerGrpcService {
    async fn send(
        &self,
        request: tonic::Request<CheckResult>,
    ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
        self.kafka
            .process_result(request.get_ref())
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        Ok(tonic::Response::new(()))
    }
}

impl BrokerGrpcService {
    pub fn new(kafka: Kafka) -> Self {
        Self { kafka }
    }

    pub async fn run_on(self, addr: std::net::SocketAddr) -> Result<()> {
        info!("Starting API server on {}", addr);
        let server = BrokerServer::new(self);
        Server::builder()
            .add_service(server)
            .serve(addr)
            .await
            .map_err(Error::ServerFailure)
    }
}
