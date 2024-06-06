use isok_data::check_kinds::http::HttpFields;
use isok_data::pulsar_messages::CheckData;
use log::{error, info};
use tokio::sync::broadcast::Receiver;
use warp10::{Client, Data as Warp10Data, Label, Value};

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
            token: connection_data.warp10_token,
        })
    }
}

pub struct Warp10HttpSink {
    warp10_client: Warp10Client,
    http_receiver: Receiver<CheckData<HttpFields>>,
}

pub fn warp10_data(check_message: &CheckData<HttpFields>, name: &str, value: Value) -> Warp10Data {
    Warp10Data::new(
        check_message.timestamp,
        None,
        name.to_string(),
        vec![
            Label::new(
                "check-id",
                check_message.check_id.as_hyphenated().to_string().as_str(),
            ),
            Label::new("agent-id", check_message.agent_id.as_str()),
        ],
        value,
    )
}

impl Warp10HttpSink {
    pub fn new(
        warp10_client: Warp10Client,
        http_receiver: Receiver<CheckData<HttpFields>>,
    ) -> Self {
        Self {
            warp10_client,
            http_receiver,
        }
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
        self.warp10_client
            .client
            .get_writer(self.warp10_client.token.clone())
            .post(data)
            .await
            .map_err(|e| {
                error!("Unable to write to warp10 {:?}", e);
                e
            })
            .ok()
            .map(|_| ())
    }

    pub async fn run(mut self) -> Option<()> {
        info!("Started warp10 sink");
        loop {
            while let Some(check_data) = self
                .http_receiver
                .recv()
                .await
                .map_err(|e| {
                    error!("reciever run into an error : {:?}", e);
                    ()
                })
                .ok()
            {
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
            }
            info!("Restarting http warp10 loop");
        }
    }
}
