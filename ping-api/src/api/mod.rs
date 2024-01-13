pub mod auth;
pub mod checks;
pub mod errors;
pub mod routes;

use crate::db::DbHandler;

pub struct ApiHandler {
    pub db: DbHandler,
}
