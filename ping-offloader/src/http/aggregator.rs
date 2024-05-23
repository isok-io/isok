use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use log::{error, info};
use pulsar::producer::Message;
use pulsar::{DeserializeMessage, Error, Payload, Producer, SerializeMessage, TokioExecutor};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime};
use tokio::sync::broadcast::Receiver;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;
use ping_data::check_kinds::http::HttpFields;
use ping_data::pulsar_messages::{CheckData, CheckMessage, CheckResult};


#[derive(Serialize, Deserialize)]
pub struct AggregatedCheckMessage {
    check_id: Uuid,
    timestamp: OffsetDateTime,
    latency: Duration,
    status_codes: StatusCodeCount,
}

impl SerializeMessage for AggregatedCheckMessage {
    fn serialize_message(input: Self) -> Result<Message, Error> {
        let payload = serde_json::to_vec(&input).map_err(|e| Error::Custom(e.to_string()))?;

        Ok(Message {
            payload,
            ..Default::default()
        })
    }
}

impl DeserializeMessage for AggregatedCheckMessage {
    type Output = Result<Self, pulsar::Error>;

    fn deserialize_message(payload: &Payload) -> Self::Output {
        serde_json::from_slice(payload.data.as_slice())
            .map_err(|e| pulsar::Error::Custom(e.to_string()))
    }
}

pub struct AggregateValues {
    pub latency: Duration,
    pub status_codes: StatusCodeCount,
}

impl AggregateValues {
    pub fn aggregate(&mut self, latency: Duration, status_code: u16) {
        self.latency = self.latency.max(latency);
        match status_code {
            0..=299 => self.status_codes._200 += 1,
            300..=399 => self.status_codes._300 += 1,
            400..=499 => self.status_codes._400 += 1,
            500..=599 => self.status_codes._500 += 1,
            _ => {}
        }
    }
}

impl Default for AggregateValues {
    fn default() -> Self {
        Self {
            latency: Duration::from_nanos(0),
            status_codes: StatusCodeCount::default(),
        }
    }
}

#[derive(Default, Copy, Clone, Serialize, Deserialize)]
pub struct StatusCodeCount {
    pub _200: usize,
    pub _300: usize,
    pub _400: usize,
    pub _500: usize,
}

pub struct AggregateBuffer {
    pub timestamp: OffsetDateTime,
    pub responded_agents: Vec<String>,
    pub aggregated_message: AggregateValues,
}

impl AggregateBuffer {
    fn default(timestamp: OffsetDateTime) -> Self {
        Self {
            timestamp,
            responded_agents: Vec::new(),
            aggregated_message: AggregateValues::default(),
        }
    }

    fn add_check(&mut self, check_data: &CheckData<HttpFields>) {
        self.responded_agents.push(check_data.agent_id.clone());
        self.aggregated_message.aggregate(check_data.latency, check_data.fields.status_code)
    }
}

pub struct Aggregator {
    http_receiver: Receiver<CheckData<HttpFields>>,
    pulsar_sink: Producer<TokioExecutor>,
    buffer: HashMap<Uuid, AggregateBuffer>,
}

impl Aggregator {
    pub fn new(http_receiver: Receiver<CheckData<HttpFields>>,
               pulsar_sink: Producer<TokioExecutor>) -> Self {
        Self {
            http_receiver,
            pulsar_sink,
            buffer: Default::default(),
        }
    }

    pub async fn run(&mut self) {
        info!("Started aggregator sink");
        while let Some(check_data) = self.http_receiver.recv().await.ok() {
            if let Some(check_buffer) = self.buffer.get_mut(&check_data.check_id) {
                if !check_buffer.responded_agents.contains(&check_data.agent_id) {
                    info!("Got data from a new agent, appending...");
                    check_buffer.add_check(&check_data)
                } else {
                    info!("Sending data to pulsar...");
                    let _ = self.pulsar_sink.send(
                        AggregatedCheckMessage {
                            check_id: check_data.check_id,
                            timestamp: check_buffer.timestamp,
                            latency: check_buffer.aggregated_message.latency,
                            status_codes: check_buffer.aggregated_message.status_codes.clone(),
                        }
                    ).await;
                }
            } else {
                info!("Got data from a new check, inserting a new buffer...");
                let mut value = AggregateBuffer::default(check_data.timestamp);
                value.add_check(&check_data);
                self.buffer.insert(check_data.check_id, value);
            }
        }
        info!("Stopped aggregator sink");
    }
}
