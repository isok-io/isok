pub use std::sync::Arc;

pub use argon2::password_hash::{Encoding, PasswordHashString};
pub use axum::extract::{Path, State};
pub use axum::response::IntoResponse;
pub use axum::{Extension, Json};
use log::info;
pub use serde::{Deserialize, Serialize};
use sqlx::Error;
pub use uuid::Uuid;

use isok_data::owner::UserOutput;
pub use isok_data::owner::{User, UserInput};

use crate::api::errors::DbQueryError;
pub use crate::api::errors::{
    Forbidden, InvalidInput, NotFoundError, PasswordHashError, WrongCredentials,
};
use crate::api::ServerState;
pub use crate::utils::validator::{valid_email, valid_password, valid_user_input, valid_username};

#[derive(Serialize, Deserialize, Debug)]
pub struct UsernameInput {
    username: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EmailInput {
    email_address: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PasswordInput {
    old_password: String,
    new_password: String,
    confirm_password: String,
}

pub async fn list_users(
    State(state): State<ServerState>,
) -> Result<Json<Vec<UserOutput>>, impl IntoResponse> {
    state.db.get_users().await.map(|users| {
        users
            .into_iter()
            .map(|user| {
                info!("{user:#?}");
                user.into()
            })
            .collect::<Vec<UserOutput>>()
            .into()
    })
}

pub async fn get_user(
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
) -> Result<Json<UserOutput>, impl IntoResponse> {
    match state.db.get_user(id).await {
        Ok(user) => Ok(<User as Into<UserOutput>>::into(user).into()),
        Err(DbQueryError(Error::RowNotFound)) => Err(NotFoundError {
            model: "user",
            value: id,
        }
        .into_response()),
        Err(e) => Err(e.into_response()),
    }
}

pub async fn create_user(
    State(state): State<ServerState>,
    Json(user): Json<UserInput>,
) -> Result<(), impl IntoResponse> {
    if let Err(e) = valid_user_input(user.clone()) {
        return Err(e.into_response());
    }

    let mut user = user;
    user.password = match crate::utils::auth::password::hash_password(
        user.password.as_str(),
        &state.argon2_params,
    ) {
        Ok(password) => password.to_string(),
        Err(e) => return Err(e.into_response()),
    };

    state
        .db
        .insert_user(user)
        .await
        .map_err(|e| e.into_response())
}

pub async fn rename_user(
    Extension(current_user): Extension<User>,
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Json(username): Json<UsernameInput>,
) -> Result<(), impl IntoResponse> {
    if id != current_user.user_id {
        return Err(Forbidden.into_response());
    }

    if let Err(e) =
        valid_username(username.username.clone()).map_err(|e| e.with_field_name("username"))
    {
        return Err(e.into_response());
    }

    state
        .db
        .update_user_username(id, username.username)
        .await
        .map_err(|e| match e {
            DbQueryError(Error::RowNotFound) => NotFoundError {
                model: "user",
                value: id,
            }
            .into_response(),
            e => e.into_response(),
        })
}

pub async fn change_user_email(
    Extension(current_user): Extension<User>,
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Json(email): Json<EmailInput>,
) -> Result<(), impl IntoResponse> {
    if id != current_user.user_id {
        return Err(Forbidden.into_response());
    }

    if let Err(e) =
        valid_email(email.email_address.clone()).map_err(|e| e.with_field_name("email_address"))
    {
        return Err(e.into_response());
    }

    state
        .db
        .update_user_email(id, email.email_address)
        .await
        .map_err(|e| match e {
            DbQueryError(Error::RowNotFound) => NotFoundError {
                model: "user",
                value: id,
            }
            .into_response(),
            e => e.into_response(),
        })
}

pub async fn change_user_password(
    Extension(current_user): Extension<User>,
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Json(password): Json<PasswordInput>,
) -> Result<(), impl IntoResponse> {
    if id != current_user.user_id {
        return Err(Forbidden.into_response());
    }

    valid_password(password.old_password.clone())
        .map_err(|e| e.with_field_name("old_password").into_response())
        .and(
            PasswordHashString::parse(current_user.password.as_str(), Encoding::B64)
                .map_err(|e| PasswordHashError(e).into_response()),
        )
        .and_then(|password_hash_string: PasswordHashString| {
            if crate::utils::auth::password::verify_password(
                password.old_password.as_str(),
                &password_hash_string,
                &state.argon2_params,
            ) {
                Ok(())
            } else {
                Err(WrongCredentials.into_response())
            }
        })?;

    valid_password(password.new_password.clone())
        .map_err(|e| e.with_field_name("new_password").into_response())?;

    if password.new_password != password.confirm_password {
        return Err(InvalidInput {
            field_name: "confirm_password",
            reason: "Passwords don't match",
        }
        .into_response());
    }

    let password = match crate::utils::auth::password::hash_password(
        password.new_password.as_str(),
        &state.argon2_params,
    ) {
        Ok(password) => password.to_string(),
        Err(e) => return Err(e.into_response()),
    };

    state
        .db
        .update_user_password(id, password)
        .await
        .map_err(|e| match e {
            DbQueryError(Error::RowNotFound) => NotFoundError {
                model: "user",
                value: id,
            }
            .into_response(),
            e => e.into_response(),
        })
}

pub async fn delete_user(
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Extension(current_user): Extension<User>,
) -> Result<(), impl IntoResponse> {
    if id != current_user.user_id {
        return Err(Forbidden.into_response());
    }

    state.db.delete_user(id).await.map_err(|e| match e {
        DbQueryError(Error::RowNotFound) => NotFoundError {
            model: "user",
            value: id,
        }
        .into_response(),
        e => e.into_response(),
    })
}
