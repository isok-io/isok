use pulsar::{DeserializeMessage, Payload, producer, SerializeMessage};
use pulsar::Error as PulsarError;
use pulsar::producer::Message;
use uuid::Uuid;

use crate::check::{Check, CheckOutput, Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCommand {
    pub check: CheckOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandKind {
    Add(AddCommand),
    Remove(Uuid),
}

pub enum RedisCommand {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
}

impl SerializeMessage for Command {
    fn serialize_message(input: Self) -> Result<Message, pulsar::Error> {
        serde_json::to_vec(&input)
            .map(|json| producer::Message {
                partition_key: Some(input.id.to_string()),
                payload: json,
                ..Default::default()
            })
            .map_err(|e| PulsarError::Custom(e.to_string()))
    }
}

impl DeserializeMessage for Command {
    type Output = Result<Command, serde_json::Error>;

    fn deserialize_message(payload: &Payload) -> Self::Output {
        serde_json::from_slice(&payload.data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    id: Uuid,
    kind: CommandKind,
}

impl Command {
    pub fn new_add_command(check: Check) -> Self {
        Self {
            id: check.check_id,
            kind: CommandKind::Add(AddCommand {
                check: check.into(),
            }),
        }
    }

    pub fn new_remove_command(check: Check) -> Self {
        Self {
            id: check.check_id,
            kind: CommandKind::Remove(check.check_id),
        }
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn kind(&self) -> &CommandKind {
        &self.kind
    }
}
