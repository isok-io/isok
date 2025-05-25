use crate::models::NameSchema;
use crate::models::{OrgName, Tags, UserSimpleView};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
pub struct OrganisationInput {
    #[schemars(with = "NameSchema<0>")]
    pub name: OrgName,
}

pub type OrganisationNameInput = OrganisationInput;

pub struct Organisation {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<Uuid>,
    pub tags: Tags,
}

#[derive(Serialize, JsonSchema)]
pub struct OrganisationSimpleView {
    pub id: Uuid,
    pub name: String,
    pub tags: Tags,
}

impl From<Organisation> for OrganisationSimpleView {
    fn from(org: Organisation) -> Self {
        Self {
            id: org.id,
            name: org.name,
            tags: org.tags,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct OrganisationView {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<UserSimpleView>,
    pub tags: Tags,
}
