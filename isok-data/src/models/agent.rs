use crate::models::Tags;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Agent {
    pub id: String,
    pub zone: Uuid,
    pub endpoint: String,
    pub token: String,
    pub added_at: DateTime<Utc>,
    pub healthchecked_at: DateTime<Utc>,
    pub tags: Tags,
}
