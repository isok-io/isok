use log::{error, info};
use isok_data::{check::Check, pulsar_commands::Command};
use pulsar::{
    compression::Compression, executor::TokioExecutor, proto, Authentication, Producer,
    ProducerOptions, Pulsar,
};

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

pub struct PulsarClient {
    pub producer: Producer<TokioExecutor>,
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

        let producer = client
            .producer()
            .with_topic(pulsar_link(&connection_data))
            .with_options(ProducerOptions {
                schema: Some(proto::Schema {
                    r#type: proto::schema::Type::String as i32,
                    ..Default::default()
                }),
                compression: Some(Compression::Zstd(pulsar::compression::CompressionZstd {
                    ..Default::default()
                })),
                ..Default::default()
            })
            .build()
            .await
            .ok()?;

        Some(PulsarClient { producer })
    }

    pub async fn add_check(&mut self, check: Check) {
        let a = self
            .producer
            .send_non_blocking(Command::new_add_command(check.clone()))
            .await;
        match a {
            Ok(a) => match a.await {
                Ok(_) => info!("Check {} sent to agent !", check.check_id),
                Err(_) => error!("Check {} could not be sent to agent.", check.check_id),
            },
            Err(_) => error!("Check {} could not be sent to agent.", check.check_id),
        };
    }

    pub async fn remove_check(&mut self, check: Check) {
        let a = self
            .producer
            .send_non_blocking(Command::new_remove_command(check.clone()))
            .await;
        match a {
            Ok(_) => match a {
                Ok(_) => info!("Check {} deleted from agent !", check.check_id),
                Err(_) => error!("Check {} could not be deleted from agent.", check.check_id),
            },
            Err(_) => error!("Check {} could not be deleted from agent.", check.check_id),
        }
    }
}
