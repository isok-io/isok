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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrganizationUserRole {
    Owner,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRole {
    pub role: OrganizationUserRole,
    #[serde(flatten)]
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleOutput {
    pub role: OrganizationUserRole,
    #[serde(flatten)]
    pub user: UserOutput,
}

impl Into<UserRoleOutput> for UserRole {
    fn into(self) -> UserRoleOutput {
        UserRoleOutput {
            role: self.role,
            user: self.user.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalOrganization {
    pub name: String,
    pub users: Vec<UserRole>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NormalOrganizationOutput {
    pub name: String,
    pub users: Vec<UserRoleOutput>,
}

impl Into<NormalOrganizationOutput> for NormalOrganization {
    fn into(self) -> NormalOrganizationOutput {
        NormalOrganizationOutput {
            name: self.name,
            users: self.users.into_iter().map(|u| u.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrganizationType {
    UserOrganization(UserOrganization),
    NormalOrganization(NormalOrganization),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum OrganizationTypeOutput {
    UserOrganization,
    NormalOrganization(NormalOrganizationOutput),
}

impl Into<OrganizationTypeOutput> for OrganizationType {
    fn into(self) -> OrganizationTypeOutput {
        match self {
            OrganizationType::UserOrganization(_) => OrganizationTypeOutput::UserOrganization,
            OrganizationType::NormalOrganization(NormalOrganization { name, users }) => {
                OrganizationTypeOutput::NormalOrganization(NormalOrganizationOutput {
                    name,
                    users: users.into_iter().map(|u| u.into()).collect(),
                })
            }
        }
    }
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

#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationOutput {
    pub organization_id: Uuid,
    pub tags: HashMap<String, Option<String>>,
    #[serde(flatten)]
    pub organization_type: OrganizationTypeOutput,
}

impl Into<OrganizationOutput> for Organization {
    fn into(self) -> OrganizationOutput {
        OrganizationOutput {
            organization_id: self.organization_id,
            tags: self.tags,
            organization_type: self.organization_type.into(),
        }
    }
}
