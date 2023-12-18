use time::OffsetDateTime;
use tokio::sync::mpsc::Receiver;
use uuid::Uuid;
use warp10::Client;

/// Warp10 data
pub use warp10::Data;
/// Warp10 data label
pub use warp10::Label;
/// Warp10 data value
pub use warp10::Value;

/// Warp10 connection data, passed by env vars
#[derive(Debug, Clone)]
pub struct Warp10ConnectionData {
    pub warp10_address: String,
    pub warp10_token: String,
}

/// Warp10 client
pub struct Warp10Client {
    pub client: Client,
    pub warp10_token: String,
}

impl Warp10Client {
    pub async fn new(connection_data: Warp10ConnectionData) -> Option<Self> {
        let client = Client::new(connection_data.warp10_address.as_str()).ok()?;

        Some(Self {
            client,
            warp10_token: connection_data.warp10_token,
        })
    }

    /// Send warp10 data
    pub async fn send(&self, datas: Vec<Data>) -> Option<()> {
        self.client
            .get_writer(self.warp10_token.clone())
            .post(datas)
            .await
            .ok()
            .map(|_| ())
    }
}

/// Data that can be send to warp10, result of a [Job](crate::job::Job) execution
pub trait Warp10Data {
    /// Make data that can be send to warp10 after a [Job](crate::job::Job) execution
    fn data(&self, uuid: Uuid) -> Vec<Data>;
}

/// Helper to make warp10 data, (used in [`Warp10Data`] impl)
pub fn warp10_data(datetime: OffsetDateTime, name: &str, uuid: Uuid, value: Value) -> Data {
    Data::new(
        datetime,
        None,
        name.to_string(),
        vec![Label::new(
            "check",
            uuid.as_hyphenated().to_string().as_str(),
        )],
        value,
    )
}

/// Warp10 writter main loop
pub async fn warp10_sender(
    warp10_client: Warp10Client,
    mut rcv: Receiver<Data>,
    send_ratio: usize,
) {
    let mut datas = Vec::new();
    let mut send_cursor = 0;

    while let Some(d) = rcv.recv().await {
        datas.push(d);

        if send_cursor == send_ratio {
            send_cursor = 0;
            warp10_client.send(datas).await;
            datas = Vec::new();
        } else {
            send_cursor += 1;
        }
    }
}
