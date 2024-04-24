use std::collections::HashMap;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;
use ping_data::pulsar_messages::{CheckData, CheckMessage};
use crate::PulsarClient;

pub async fn pulsar_sink(pulsar_client: PulsarClient, mut receiver: Receiver<CheckData>) {
    while let Some(check_result) = receiver.recv().await {
        pulsar_client.send(check_result).await
    }
}