use crate::config::Config;

mod api;
pub mod config;
mod errors;
mod kafka;

use crate::api::BrokerGrpcService;
use crate::kafka::Kafka;
use errors::Result;

pub async fn run(config: Config) -> Result<()> {
    let kafka = Kafka::new(config.kafka)?;

    BrokerGrpcService::new(kafka)
        .run_on(config.api.listen_address)
        .await?;

    Ok(())
}
