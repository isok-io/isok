use log::{error, info};
use futures::TryStreamExt;
use pulsar::{Authentication, Consumer, ConsumerOptions, Pulsar, SubType, TokioExecutor};
use ping_data::pulsar_messages::{CheckData, CheckMessage};
use warp10::{Client, Data as Warp10Data, Label, Value};
use ping_data::check_kinds::http::HttpFields;


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
        connection_data.pulsar_tenant,
        connection_data.pulsar_namespace,
    )
}

pub struct PulsarHttpSource {
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

        info!("Starting consumer with subscription id : {}", &connection_data.subscription_uuid);

        let consumer =
            client.consumer()
                .with_topic(pulsar_http_topic(connection_data))
                .with_subscription_type(SubType::Failover)
                .with_consumer_name("warp10-http-sink")
                .with_subscription(&connection_data.subscription_uuid)
                .with_options(ConsumerOptions {
                    durable: Some(true),
                    read_compacted: Some(true),
                    ..Default::default()
                })
                .build()
                .await
                .ok()?;

        Some(Self { consumer })
    }
}

/// Warp10 connection data, passed by env vars
#[derive(Debug, Clone)]
pub struct Warp10ConnectionData {
    pub warp10_address: String,
    pub warp10_token: String,
}


pub struct Warp10Client {
    client: Client,
    token: String,
}

impl Warp10Client {
    pub fn new(connection_data: Warp10ConnectionData) -> Option<Self> {
        Some(Self {
            client: Client::new(connection_data.warp10_address.as_str()).ok()?,
            token: connection_data.warp10_address,
        })
    }
}

pub struct Warp10HttpSink {
    warp10_client: Warp10Client,
    pulsar_http_source: PulsarHttpSource,
}

pub fn warp10_data(check_message: &CheckData<HttpFields>, name: &str, value: Value) -> Warp10Data {
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
    pub fn new(warp10_client: Warp10Client, pulsar_http_source: PulsarHttpSource) -> Self {
        Self { warp10_client, pulsar_http_source }
    }

    pub fn data(check_data: CheckData<HttpFields>) -> Vec<Warp10Data> {
        vec![
            warp10_data(
                &check_data,
                "http.request_duration",
                Value::Long(check_data.latency.as_millis() as i64),
            ),
            warp10_data(
                &check_data,
                "http.request_status",
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

    pub async fn run(mut self) -> Option<()> {
        info!("Started warp10 sink");
        while let Some(message) =
            self
                .pulsar_http_source.consumer
                .try_next()
                .await
                .map_err(|e| {
                    error!("Unable to read from pulsar {:?}", e);
                    e
                })
                .ok()? {
            let check_data: CheckData<HttpFields> = match message.deserialize() {
                Ok(data) => {
                    info!("Received a message from {} of check {}", data.agent_id, data.check_id);
                    data.into()
                }
                Err(e) => {
                    error!("Could not deserialize message: {:?}", e);
                    let _ = self.pulsar_http_source.consumer.ack(&message).await;
                    continue;
                }
            };

            let warp10_data = Self::data(check_data);
            let _ = match self.send(warp10_data).await {
                None => {
                    error!("Failed to send data to warp10");
                    continue;
                }
                Some(_) => {
                    info!("Sent data to warp10")
                }
            };

            let _ = self.pulsar_http_source.consumer.ack(&message).await;
        }
        info!("Stopped warp10 sink");
        Some(())
    }
}

