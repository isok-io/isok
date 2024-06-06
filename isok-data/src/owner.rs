use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::check::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
    pub email_address: String,
    pub self_organization: Organization,
    pub tags: HashMap<String, Option<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInput {
    pub username: String,
    pub password: String,
    pub email_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOutput {
    pub owner_id: Uuid,
    pub username: String,
    pub email_address: String,
}

impl Into<User> for UserInput {
    fn into(self) -> User {
        let timestamp = Utc::now();
        User {
            user_id: Uuid::new_v4(),
            username: self.username.clone(),
            password: self.password,
            email_address: self.email_address,
            self_organization: Organization {
                organization_id: Uuid::new_v4(),
                tags: Default::default(),
                organization_type: OrganizationType::UserOrganization(UserOrganization),
                created_at: timestamp,
                updated_at: timestamp,
                deleted_at: None,
            },
            tags: Default::default(),
            created_at: timestamp,
            updated_at: timestamp,
            deleted_at: None,
        }
    }
}

impl Into<UserOutput> for User {
    fn into(self) -> UserOutput {
        UserOutput {
            owner_id: self.user_id,
            username: self.username,
            email_address: self.email_address,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOrganization;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalOrganization {
    pub name: String,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrganizationType {
    UserOrganization(UserOrganization),
    NormalOrganization(NormalOrganization),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub organization_id: Uuid,
    pub tags: HashMap<String, Option<String>>,
    pub organization_type: OrganizationType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}
