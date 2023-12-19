use std::sync::{Arc, Mutex};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use crate::utils::{env_get, env_get_num};

pub mod routes;
pub mod auth;
pub mod users;
pub mod checks;
mod utils;
mod errors;

#[tokio::main]
async fn main() {
    let postgresql_uri = env_get("POSTGRESQL_ADDON_URI");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&postgresql_uri).await.unwrap();

    let app = routes::app(pool);

    let listener =
        tokio::net::TcpListener::bind("localhost:8080").await.unwrap();

    axum::serve(listener, app).await.unwrap()
}
