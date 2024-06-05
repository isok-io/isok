use futures::TryStreamExt;
use isok_data::pulsar_messages::{CheckData, CheckMessage, CheckType};
use log::{error, info};
use pulsar::{Authentication, Consumer, ConsumerOptions, Pulsar, SubType, TokioExecutor};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use tokio::sync::broadcast::{error::SendError, Sender};

/// Pulsar connection data, passed by env vars
#[derive(Debug, Clone)]
pub struct PulsarConnectionData {
    pub pulsar_address: String,
    pub pulsar_token: String,
    pub pulsar_tenant: String,
    pub pulsar_namespace: String,
    pub subscription_uuid: String,
}

pub fn pulsar_http_topic(connection_data: &PulsarConnectionData) -> String {
    format!(
        "persistent://{}/{}/http",
        connection_data.pulsar_tenant, connection_data.pulsar_namespace,
    )
}

pub struct PulsarSource<A: Serialize + DeserializeOwned + Debug> {
    consumer: Consumer<CheckMessage, TokioExecutor>,
    sender: Sender<CheckData<A>>,
}

impl<A: Serialize + DeserializeOwned + Debug> PulsarSource<A> {
    pub async fn new(
        connection_data: &PulsarConnectionData,
        sender: Sender<CheckData<A>>,
        check_type: CheckType,
    ) -> Option<Self> {
        let client = Pulsar::builder(&connection_data.pulsar_address, TokioExecutor)
            .with_auth(Authentication {
                name: "token".to_owned(),
                data: Vec::from(connection_data.pulsar_token.as_bytes()),
            })
            .build()
            .await
            .ok()?;

        let subscription_name = format!("{}-{}", &connection_data.subscription_uuid, check_type);

        info!(
            "Starting consumer with subscription id : {}",
            &subscription_name
        );

        let consumer = client
            .consumer()
            .with_topic(pulsar_http_topic(connection_data))
            .with_subscription_type(SubType::Failover)
            .with_consumer_name(&subscription_name)
            .with_subscription(&subscription_name)
            .with_options(ConsumerOptions {
                durable: Some(true),
                read_compacted: Some(true),
                ..Default::default()
            })
            .build()
            .await
            .ok()?;

        Some(Self { consumer, sender })
    }

    pub async fn run(&mut self) {
        loop {
            while let Some(message) = self
                .consumer
                .try_next()
                .await
                .map_err(|e| {
                    error!("Unable to read from pulsar {:?}", e);
                    e
                })
                .ok()
                .flatten()
            {
                info!("Received a message from pulsar");

                let check_data: CheckData<A> = match message.deserialize() {
                    Ok(data) => {
                        info!(
                            "Received a message from {} of check {}",
                            data.agent_id, data.check_id
                        );
                        data.into()
                    }
                    Err(e) => {
                        error!("Could not deserialize message... skipping : {:?}", e);
                        let _ = self.consumer.ack(&message).await;
                        continue;
                    }
                };

                let mut check_data = Some(check_data);
                while let Some(data) = check_data.take() {
                    check_data = match self.sender.send(data) {
                        Ok(_) => {
                            _ = self.consumer.ack(&message).await.map_err(|_| {
                            error!("Pulsar consumer failed to acknowledge check data while sending it to broadcast channel");
                        });
                            None
                        }
                        Err(SendError(returned_data)) => {
                            error!(
                                "Could not send data to channel : {:?} (channel len : {})",
                                &returned_data,
                                &self.sender.len()
                            );
                            Some(returned_data)
                        }
                    };
                }
            }
        }
    }
}
