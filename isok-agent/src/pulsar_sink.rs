use std::time::Duration;
use pulsar::{Producer, TokioExecutor};
use time::OffsetDateTime;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;
use isok_data::check::{Deserialize, Serialize};

use isok_data::pulsar_messages::CheckMessage;

pub async fn pulsar_sink(mut producer: Producer<TokioExecutor>, mut receiver: Receiver<CheckMessage>) {
    while let Some(check_msg) = receiver.recv().await {
        let _ = producer.send(check_msg).await;
    }
}