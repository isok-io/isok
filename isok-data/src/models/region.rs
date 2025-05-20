use crate::models::Tags;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, JsonSchema)]
pub struct Region {
    pub id: Uuid,
    pub name: String,
    pub zones: Vec<Zone>,
    pub tags: Tags,
}

#[derive(Serialize, JsonSchema)]
pub struct Zone {
    pub id: Uuid,
    pub name: String,
    pub tags: Tags,
}
