use std::collections::HashMap;
use std::fmt::{Display, Formatter, write};
use std::str::FromStr;
use std::time::Duration;
use pulsar::producer::Message;
use pulsar::{Error, SerializeMessage};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct CheckMessage {
    pub check_id: Uuid,
    pub agent_id: String,
    pub timestamp: OffsetDateTime,
    pub latency: Duration,
    pub fields: HashMap<String, String>,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CheckType {
    Http
}

impl Display for CheckType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            CheckType::Http => "http"
        })
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct CheckTypeParseError(String);

impl Display for CheckTypeParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid CheckType value, should be a lowercase string, got : {}", self.0)
    }
}

impl std::error::Error for CheckTypeParseError {}

impl FromStr for CheckType {
    type Err = CheckTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(CheckType::Http),
            _ => Err(CheckTypeParseError(s.to_string()))
        }
    }
}

pub struct CheckResult {
    pub timestamp: OffsetDateTime,
    pub latency: Duration,
    pub fields: HashMap<String, String>,
}

impl CheckResult {
    pub fn to_message(&self, check_id: Uuid, agent_id: String) -> CheckMessage {
        CheckMessage {
            check_id,
            agent_id,
            timestamp: self.timestamp,
            latency: self.latency,
            fields: self.fields.clone(),
        }
    }
}