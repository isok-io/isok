use chrono::{DateTime, FixedOffset};
use pulsar::producer::Message;
use pulsar::{DeserializeMessage, Error, Payload, SerializeMessage};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckMessage {
    pub check_id: Uuid,
    pub agent_id: String,
    pub timestamp: DateTime<FixedOffset>,
    /// Latency in milliseconds
    pub latency: u64,
    pub fields: serde_json::Value,
}

impl SerializeMessage for CheckMessage {
    fn serialize_message(input: Self) -> Result<Message, Error> {
        let payload = serde_json::to_vec(&input).map_err(|e| Error::Custom(e.to_string()))?;

        Ok(Message {
            payload,
            ..Default::default()
        })
    }
}

impl DeserializeMessage for CheckMessage {
    type Output = Result<CheckMessage, pulsar::Error>;

    fn deserialize_message(payload: &Payload) -> Self::Output {
        serde_json::from_slice(payload.data.as_slice())
            .map_err(|e| pulsar::Error::Custom(e.to_string()))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CheckType {
    Http,
}

impl Display for CheckType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                CheckType::Http => "http",
            }
        )
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct CheckTypeParseError(String);

impl Display for CheckTypeParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid CheckType value, should be a lowercase string, got : {}",
            self.0
        )
    }
}

impl std::error::Error for CheckTypeParseError {}

impl FromStr for CheckType {
    type Err = CheckTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(CheckType::Http),
            _ => Err(CheckTypeParseError(s.to_string())),
        }
    }
}

pub struct CheckResult<A: Serialize + DeserializeOwned> {
    pub timestamp: DateTime<FixedOffset>,
    pub latency: Duration,
    pub fields: A,
}

impl<A: Serialize + DeserializeOwned> CheckResult<A> {
    pub fn to_message(&self, check_id: Uuid, agent_id: String) -> CheckMessage {
        CheckMessage {
            check_id,
            agent_id,
            timestamp: self.timestamp,
            latency: self.latency.as_millis() as u64,
            fields: serde_json::to_value(&self.fields).unwrap(), //cannot fail
        }
    }
}

#[derive(Clone, Debug)]
pub struct CheckData<A: Serialize + DeserializeOwned + Debug> {
    pub check_id: Uuid,
    pub agent_id: String,
    pub timestamp: DateTime<FixedOffset>,
    pub latency: Duration,
    pub fields: A,
}

impl<A: Serialize + DeserializeOwned + Debug> Into<CheckData<A>> for CheckMessage {
    fn into(self) -> CheckData<A> {
        CheckData {
            check_id: self.check_id,
            agent_id: self.agent_id,
            timestamp: self.timestamp,
            latency: Duration::from_millis(self.latency),
            fields: serde_json::from_value(self.fields).unwrap(), //cannot fail
        }
    }
}
