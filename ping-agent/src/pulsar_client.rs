use log::info;
use ping_data::pulsar_commands::Command;
use pulsar::{
    consumer::InitialPosition, executor::TokioExecutor, Authentication, Consumer, ConsumerOptions,
    Pulsar, SubType,
};
use uuid::Uuid;

/// Helper to make topic link from tenant, namespace and topic
pub fn pulsar_link(connection_data: &PulsarConnectionData) -> String {
    format!(
        "persistent://{}/{}/{}",
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
        connection_data.pulsar_topic
    )
}

/// Pulsar connection data, passed by env vars
#[derive(Debug, Clone)]
pub struct PulsarConnectionData {
    pub pulsar_address: String,
    pub pulsar_token: String,
    pub pulsar_tenant: String,
    pub pulsar_namespace: String,
    pub pulsar_topic: String,
}

/// A pulsar client
pub struct PulsarClient {
    pub consumer: Consumer<Command, TokioExecutor>,
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

        Some(PulsarClient { consumer })
    }
}
