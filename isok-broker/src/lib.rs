mod api;
pub mod config;
mod transport;

use crate::config::{Config, Error};
use crate::transport::TransportLayer;

pub async fn run(config: Config) -> Result<(), Error> {
    let transport = TransportLayer::try_new(config.transport)?;

    api::BrokerGrpcService::new(transport)
        .run_on(config.api.listen_address)
        .await
        .map_err(Error::UnableToStartApiServer)?;
    Ok(())
}
