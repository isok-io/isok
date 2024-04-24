use std::collections::HashMap;
use std::sync::Arc;
use log::info;
use ping_data::pulsar_commands::Command;
use pulsar::{consumer::InitialPosition, executor::TokioExecutor, Authentication, Consumer, ConsumerOptions, Pulsar, SubType, Producer, ProducerOptions, Error};
use pulsar::producer::SendFuture;
use tokio::sync::Mutex;
use uuid::Uuid;
use ping_data::pulsar_messages::{CheckMessage, CheckType};

/// Helper to make topic link from tenant, namespace and topic
pub fn pulsar_link(connection_data: &PulsarConnectionData) -> String {
    format!(
        "persistent://{}/{}/{}",
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
        connection_data.pulsar_consumer_topic
    )
}

pub fn pulsar_producer_topic(connection_data: &PulsarConnectionData, kind: CheckType) -> String {
    format!(
        "persistent://{}/{}/{}",
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
        kind.to_string()
    )
}

/// Pulsar connection data, passed by env vars
#[derive(Debug, Clone)]
pub struct PulsarConnectionData {
    pub pulsar_address: String,
    pub pulsar_token: String,
    pub pulsar_tenant: String,
    pub pulsar_namespace: String,
    pub pulsar_consumer_topic: String,
}

/// A pulsar client
pub struct PulsarClient {
    pub client: Pulsar<TokioExecutor>,
    pub consumer: Consumer<Command, TokioExecutor>,
    pub connection_data: PulsarConnectionData,
}

impl PulsarClient {
    pub async fn new(connection_data: PulsarConnectionData) -> Option<Self> {
        let client = Pulsar::builder(&connection_data.pulsar_address, TokioExecutor)
            .with_auth(Authentication {
                name: "token".to_owned(),
                data: Vec::from(connection_data.pulsar_token.as_bytes()),
            })
            .build()
            .await
            .ok()?;

        let subscription_uuid = Uuid::new_v4().hyphenated().to_string();
        info!("Starting consumer with subscription id : {subscription_uuid}");

        let consumer: Consumer<Command, _> = client
            .consumer()
            .with_topic(pulsar_link(&connection_data))
            .with_subscription_type(SubType::Exclusive)
            .with_consumer_name("consumer")
            .with_subscription(subscription_uuid)
            .with_options(ConsumerOptions {
                read_compacted: Some(true),
                initial_position: InitialPosition::Earliest,
                ..Default::default()
            })
            .build()
            .await
            .ok()?;

        Some(PulsarClient { client, consumer, connection_data })
    }

    pub async fn create_producer(&self, kind: CheckType) -> Option<Producer<TokioExecutor>> {
        self.client.producer()
            .with_topic(pulsar_producer_topic(&self.connection_data, kind))
            .with_name(kind.to_string())
            .build()
            .await
            .ok()
    }
}

