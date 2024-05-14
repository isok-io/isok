use log::{error, info};
use tokio::sync::broadcast;
use ping_data::check_kinds::http::HttpFields;
use ping_data::pulsar_messages::CheckType;
use crate::{env_get, env_get_num};
use crate::http::warp10::{Warp10Client, Warp10ConnectionData, Warp10HttpSink};
use crate::pulsar_source::{pulsar_http_topic, PulsarConnectionData, PulsarSource};

pub mod warp10;

pub async fn run_http(pulsar_connection_data: PulsarConnectionData) {
    let warp10_address = env_get("WARP10_ADDRESS");
    let warp10_token = env_get("WARP10_TOKEN");

    let channel_capacity = env_get_num("HTTP_CHANNEL_CAPACITY", 16);

    let warp10_connection_data = Warp10ConnectionData {
        warp10_address,
        warp10_token,
    };

    let (pulsar_sender,
        warp10_receiver) =
        broadcast::channel(channel_capacity);

    info!(
        "Connecting to pulsar topic {}...",
        pulsar_http_topic(&pulsar_connection_data)
    );

    let mut pulsar_source: PulsarSource<HttpFields> = match PulsarSource::new(&pulsar_connection_data, pulsar_sender, CheckType::Http).await {
        Some(pc) => {
            info!("Connected to pulsar topic !");
            pc
        }
        None => {
            error!("Failed to connect to pulsar topic");
            std::process::exit(1);
        }
    };

    info!("Connecting to warp10...",);
    let warp10_client = match Warp10Client::new(warp10_connection_data) {
        Some(pc) => {
            info!("Connected to warp10 !");
            pc
        }
        None => {
            error!("Failed to connect to warp10");
            std::process::exit(1);
        }
    };

    let warp10_http_sink = Warp10HttpSink::new(warp10_client, warp10_receiver);

    pulsar_source.run().await;
    warp10_http_sink.run().await;
}