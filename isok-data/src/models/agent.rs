use crate::models::Tags;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

#[derive(Clone)]
pub struct Agent {
    pub id: String,
    pub zone: Uuid,
    pub endpoint: String,
    pub token: String,
    pub added_at: DateTime<Utc>,
    pub healthchecked_at: DateTime<Utc>,
    pub tags: Tags,
}

impl Hash for Agent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.id.as_bytes())
    }
}

impl PartialEq<Self> for Agent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Agent {}

#[derive(Serialize, Deserialize)]
pub struct AgentInput {
    pub id: String,
    pub zone: Uuid,
    pub endpoint: String,
    pub token: String,
    pub tags: Tags,
}

impl From<AgentInput> for Agent {
    fn from(val: AgentInput) -> Self {
        let now = Utc::now();

        Agent {
            id: val.id,
            zone: val.zone,
            endpoint: val.endpoint,
            token: val.token,
            added_at: now,
            healthchecked_at: now,
            tags: val.tags,
        }
    }
}

#[derive(Serialize)]
pub struct AgentView {
    pub id: String,
    pub zone: Uuid,
    pub added_at: DateTime<Utc>,
    pub healthchecked_at: DateTime<Utc>,
    pub tags: Tags,
}

impl From<Agent> for AgentView {
    fn from(value: Agent) -> Self {
        Self {
            id: value.id,
            zone: value.zone,
            added_at: value.added_at,
            healthchecked_at: value.healthchecked_at,
            tags: value.tags,
        }
    }
}

#[derive(Serialize)]
pub struct AgentDetailsView {
    #[serde(flatten)]
    pub view: AgentView,
    pub checks: Vec<Uuid>,
}
