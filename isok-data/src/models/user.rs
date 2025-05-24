use crate::models::EmailRegex;
use crate::models::PasswordSchema;
use crate::models::{Email, Password, Tags};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
pub struct Creds {
    #[schemars(with = "EmailRegex")]
    pub email: Email,
    #[schemars(with = "PasswordSchema")]
    pub password: Password,
}

pub type UserInput = Creds;

#[derive(Deserialize, JsonSchema)]
pub struct PatchUser {
    #[schemars(with = "Option<EmailRegex>")]
    pub email: Option<Email>,
    #[schemars(with = "Option<PasswordSchema>")]
    pub password: Option<Password>,
}

#[derive(Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password: String,
    pub tags: Tags,
}

#[derive(Serialize, JsonSchema)]
pub struct UserSimpleView {
    pub id: Uuid,
    pub email: String,
}

impl From<User> for UserSimpleView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct UserView {
    pub id: Uuid,
    pub email: String,
    pub tags: Tags,
}

impl From<User> for UserView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            tags: user.tags,
        }
    }
}

#[derive(Serialize, JsonSchema)]
pub struct Token {
    pub token: String,
    pub user_id: Uuid,
}
