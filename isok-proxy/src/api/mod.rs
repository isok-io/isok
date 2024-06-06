pub use std::collections::HashMap;
use std::sync::Arc;

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
pub struct ServerState {
    pub private_key: Arc<PrivateKey>,
    pub argon2_params: Arc<Params>,
    pub db: Arc<DbHandler>,
    pub apis: Arc<HashMap<String, Uri>>,
}
