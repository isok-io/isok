use log::info;
use pulsar::{Producer, TokioExecutor};
use tokio::sync::mpsc::Receiver;

use ping_data::pulsar_messages::CheckMessage;

pub async fn pulsar_sink(mut producer: Producer<TokioExecutor>, mut receiver: Receiver<CheckMessage>) {
    while let Some(check_msg) = receiver.recv().await {
        info!(
            "received a message of id: {}", check_msg.check_id
        );

        producer.send(check_msg).await;
    }
}