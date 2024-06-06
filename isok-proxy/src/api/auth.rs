pub use argon2::password_hash::{Encoding, PasswordHashString};
use axum::extract::State;
pub use axum::response::IntoResponse;
pub use axum::Json;
pub use serde::Deserialize;
pub use uuid::Uuid;

use crate::api::errors::BiscuitError;
pub use crate::api::errors::{PasswordHashError, WrongCredentials};
use crate::api::ServerState;
pub use crate::utils::auth::password::verify_password;
pub use crate::utils::auth::token::build_token;
pub use crate::utils::validator::{valid_email, valid_password};

#[derive(Deserialize)]
pub struct Login {
    pub login: String,
    pub password: String,
}

pub struct UserPassword {
    pub user_id: Uuid,
    pub password: String,
}

pub async fn login_handler(
    State(state): State<ServerState>,
    Json(payload): Json<Login>,
) -> Result<String, impl IntoResponse> {
    valid_email(payload.login.clone())
        .map_err(|e| e.with_field_name("login"))
        .and(valid_password(payload.password.clone()).map_err(|e| e.with_field_name("password")))
        .map_err(|e| e.into_response())?;

    let user_password = match state
        .db
        .get_user_password(payload.login)
        .await
        .map_err(|e| e.into_response())
    {
        Ok(password) => password,
        Err(e) => {
            return Err(e);
        }
    };

    let password_hash_string =
        match PasswordHashString::parse(user_password.password.as_str(), Encoding::B64)
            .map_err(|e| PasswordHashError(e).into_response())
        {
            Ok(password) => password,
            Err(e) => return Err(e),
        };

    if verify_password(
        payload.password.as_str(),
        &password_hash_string,
        &state.argon2_params,
    ) {
        build_token(user_password.user_id, &state.private_key).map_err(BiscuitError::into_response)
    } else {
        Err(WrongCredentials.into_response())
    }
}

pub async fn logout_handler() {
    todo!()
}
