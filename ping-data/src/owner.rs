use chrono::NaiveDateTime;
use uuid::Uuid;
use crate::check::{Deserialize, Serialize};

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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub deleted_at: NaiveDateTime,
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



