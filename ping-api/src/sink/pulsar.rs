use std::fmt::format;
use log::{error, info};
use futures::TryStreamExt;
use pulsar::{Authentication, Consumer, ConsumerOptions, DeserializeMessage, Error, Pulsar, SubType, TokioExecutor};
use pulsar::consumer::InitialPosition;
use time::OffsetDateTime;
use uuid::Uuid;
use ping_data::pulsar_messages::{CheckData, CheckMessage};
use crate::db::DbHandler;
use crate::pulsar::PulsarConnectionData;
use warp10::{Client, Data as Warp10Data, Label, Value};
use ping_data::check_kinds::http::HttpFields;

fn pulsar_http_topic(connection_data: &PulsarConnectionData) -> String {
    format!(
        "persistent://{}/{}/http",
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
    )
}

struct PulsarHttpSource {
    consumer: Consumer<CheckMessage, TokioExecutor>,
}

impl PulsarHttpSource {
    pub async fn new(connection_data: &PulsarConnectionData) -> Option<Self> {
        let client = Pulsar::builder(&connection_data.pulsar_address, TokioExecutor)
            .with_auth(Authentication {
                name: "token".to_owned(),
                data: Vec::from(connection_data.pulsar_token.as_bytes()),
            })
            .build()
            .await
            .ok()?;

        let subscription_uuid = Uuid::new_v4().hyphenated().to_string();
        info!("Starting consumer with subscription id : {subscription_uuid}-http");

        let consumer =
            client.consumer()
                .with_topic(pulsar_http_topic(connection_data))
                .with_subscription_type(SubType::Exclusive)
                .with_consumer_name("consumer")
                .with_subscription(format!("{subscription_uuid}-http"))
                .with_options(ConsumerOptions {
                    read_compacted: Some(true),
                    initial_position: InitialPosition::Latest,
                    ..Default::default()
                })
                .build()
                .await
                .ok()?;

        Some(Self { consumer })
    }
}

struct Warp10Client {
    client: Client,
    token: String,
}

struct Warp10HttpSink {
    warp10_client: Warp10Client,
    database_client: DbHandler,
    pulsar_http_source: PulsarHttpSource,
}

pub fn warp10_data(check_message: CheckData<HttpFields>, name: &str, value: Value) -> Warp10Data {
    Warp10Data::new(
        check_message.timestamp,
        None,
        name.to_string(),
        vec![Label::new(
            "check_id",
            check_message.check_id.as_hyphenated().to_string().as_str(),
        ), Label::new(
            "agent_id",
            check_message.agent_id.as_str(),
        )],
        value,
    )
}

impl Warp10HttpSink {
    pub fn new(warp10_client: Warp10Client,
               database_client: DbHandler,
               pulsar_http_source: PulsarHttpSource) -> Self {
        Self { warp10_client, database_client, pulsar_http_source }
    }

    pub fn data(check_data: CheckData<HttpFields>) -> Vec<Warp10Data> {
        vec![
            warp10_data(
                check_data,
                "http_request_time",
                Value::Long(check_data.latency.as_millis() as i64),
            ),
            warp10_data(
                check_data,
                "http_request_status",
                Value::Int(check_data.fields.status_code as i32),
            ),
        ]
    }

    pub async fn send(&self, data: Vec<Warp10Data>) -> Option<()> {
        self.warp10_client.client
            .get_writer(self.warp10_client.token.clone())
            .post(data)
            .await.ok().map(|_| ())
    }

    pub async fn run(mut self) {
        while let Some(message) = self.pulsar_http_source.consumer.try_next().await.ok().flatten() {
            let check_data: CheckData<HttpFields> = match message.deserialize() {
                Ok(data) => data.into(),
                Err(e) => {
                    error!("could not deserialize message: {:?}", e);
                    break;
                }
            };

            let warp10_data = Self::data(check_data);
            let _ = self.send(warp10_data).await;
            let _ = self.pulsar_http_source.consumer.ack(&message).await;
        }
    }
}

