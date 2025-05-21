use crate::api::ApiState;
use axum::Json;
use axum::extract::State;

use crate::Result;
use crate::api::errors::{ApiError, ApiResult};
use crate::config::Argon2Params;
use crate::errors::Error;
use aide::axum::ApiRouter;
use aide::axum::routing::post_with;
use argon2::password_hash::Error as PasswordHashError;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use biscuit_auth::KeyPair;
use biscuit_auth::macros::biscuit;
use isok_data::models::{Creds, Token, User, UserInput};
use uuid::Uuid;

pub struct Hasher {
    params: argon2::Params,
}

impl Hasher {
    pub fn new(params: &Argon2Params) -> Self {
        Self {
            params: argon2::Params::new(*params.m_cost, *params.t_cost, *params.p_cost, None)
                .unwrap(),
        }
    }

    pub fn hash_password(&self, password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        Ok(Argon2::from(&self.params)
            .hash_password(password.as_bytes(), &salt)?
            .to_string())
    }

    fn check_password(&self, password: &str, hashed: &str) -> Result<String> {
        let hashed = PasswordHash::new(hashed)?;
        match Argon2::from(&self.params).verify_password(password.as_bytes(), &hashed) {
            Ok(_) => self.hash_password(password),
            Err(PasswordHashError::Password) => Err(Error::WrongCredentials),
            Err(e) => Err(e.into()),
        }
    }
}

fn build_token(user: Uuid, keypair: &KeyPair) -> ApiResult<String> {
    let authority = biscuit!("user({user})", user = user.as_hyphenated().to_string());
    authority
        .build(keypair)
        .map_err::<Error, _>(Into::into)?
        .to_base64()
        .map_err::<Error, _>(Into::into)
        .map_err(Into::into)
}

async fn register(
    State(state): State<ApiState>,
    Json(user): Json<UserInput>,
) -> ApiResult<Json<Token>> {
    let user_id = state
        .db
        .users_insert_user(User {
            id: Uuid::new_v4(),
            email: user.email.to_string(),
            password: state.hasher.hash_password(&user.password)?,
            tags: Default::default(),
        })
        .await
        .map_err(|error| match error {
            Error::Db(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                ApiError::bad_request("user exists".to_string())
            }
            error => error.into(),
        })?;

    let token = build_token(user_id, &state.keypair)?;

    Ok(Json(Token { token, user_id }))
}

async fn authenticate(
    State(state): State<ApiState>,
    Json(creds): Json<Creds>,
) -> ApiResult<Json<Token>> {
    let user = match state.db.users_get_by_email(&creds.email).await? {
        None => return Err(Error::WrongCredentials.into()),
        Some(user) => user,
    };

    let new_hash = state
        .hasher
        .check_password(&creds.password, &user.password)?;
    state.db.users_update_password(user.id, &new_hash).await?;

    let token = build_token(user.id, &state.keypair)?;

    Ok(Json(Token {
        token,
        user_id: user.id,
    }))
}

pub fn router(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/register",
            post_with(register, |op| {
                op.tag("auth").id("registerV1").description("Register user")
            }),
        )
        .api_route(
            "/auth",
            post_with(authenticate, |op| {
                op.tag("auth").id("authV1").description("Authenticate")
            }),
        )
        .with_state(state)
}
