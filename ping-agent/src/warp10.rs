use time::OffsetDateTime;
use uuid::Uuid;
use warp10::{Client, Data, Label, Value};

#[derive(Debug, Clone)]
pub struct Warp10ConnectionData {
    pub warp10_address: String,
    pub warp10_token: String,
}

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

    pub async fn send(&self, datas: Vec<Data>) -> Option<()> {
        self.client
            .get_writer(self.warp10_token.clone())
            .post(datas)
            .await
            .ok()
            .map(|_| ())
    }
}

pub trait Warp10Data {
    fn data(&self, uuid: Uuid) -> Vec<Data>;
}

pub fn new_data(datetime: OffsetDateTime, name: &str, uuid: Uuid, value: Value) -> Data {
    Data::new(datetime, None, name.to_string(), vec![Label::new("check", uuid.as_hyphenated().to_string().as_str())], value)
}
