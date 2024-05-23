use futures::FutureExt;
use log::{error, info};
use pulsar::{Authentication, Producer, Pulsar, TokioExecutor};
use tokio::sync::broadcast;
use ping_data::check_kinds::http::HttpFields;
use ping_data::pulsar_messages::CheckType;
use crate::{env_get, env_get_num};
use crate::http::aggregator::{AggregatedCheckMessage, Aggregator};
use crate::http::warp10::{Warp10Client, Warp10ConnectionData, Warp10HttpSink};
use crate::pulsar_source::{pulsar_http_topic, PulsarConnectionData, PulsarSource};

pub mod warp10;
pub mod aggregator;

pub fn pulsar_producer_topic(connection_data: &PulsarConnectionData) -> String {
    format!(
        "persistent://{}/{}/{}",
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
        "aggregated-http"
    )
}

pub async fn pulsar_http_aggregate_sink(connection_data: PulsarConnectionData) -> Option<Producer<TokioExecutor>> {
    let client = Pulsar::builder(&connection_data.pulsar_address, TokioExecutor)
        .with_auth(Authentication {
            name: "token".to_owned(),
            data: Vec::from(connection_data.pulsar_token.as_bytes()),
        })
        .build()
        .await
        .ok()?;

    client.producer()
        .with_topic(pulsar_producer_topic(&connection_data))
        .with_name("aggregated-http".to_string())
        .build()
        .await
        .ok()
}

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

    let aggregator_receiver = pulsar_sender.subscribe();

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

    let pulsar_aggregate_sink: Producer<TokioExecutor> = match pulsar_http_aggregate_sink(pulsar_connection_data).await {
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

    let mut aggregator_http_sink = Aggregator::new(aggregator_receiver, pulsar_aggregate_sink);

    pulsar_source.run().await;
    warp10_http_sink.run().await;
    aggregator_http_sink.run().await;
}