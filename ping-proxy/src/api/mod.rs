pub use std::collections::HashMap;

pub use argon2::Params;
pub use axum::http::Uri;
pub use biscuit_auth::PrivateKey;

pub use crate::db::DbHandler;

pub mod auth;
pub mod checks;
pub mod errors;
pub mod middlewares;
pub mod routes;
pub mod user;

#[derive(Clone)]
pub struct AuthHandler {
    pub private_key: PrivateKey,
    pub argon2_params: Params,
    pub db: DbHandler,
}

pub struct ApiHandler {
    pub apis: HashMap<String, Uri>,
}
