use crate::check::{Deserialize, Serialize};
use chrono::{DateTime, NaiveDateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Owner {
    User(User),
    Organization(Organization),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub owner_id: Uuid,
    pub username: String,
    pub password: String,
    pub email_address: String,
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
            owner_id: Uuid::new_v4(),
            username: self.username,
            password: self.password,
            email_address: self.email_address,
            created_at: timestamp,
            updated_at: timestamp,
            deleted_at: None,
        }
    }
}

impl Into<UserOutput> for User {
    fn into(self) -> UserOutput {
        UserOutput {
            owner_id: self.owner_id,
            username: self.username,
            email_address: self.email_address,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub owner_id: Uuid,
    pub name: String,
    pub users: Vec<User>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: NaiveDateTime,
}
