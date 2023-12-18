use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use crate::utils::{env_get, env_get_num};

pub mod routes;
pub mod auth;
pub mod users;
pub mod checks;
mod utils;

#[tokio::main]
async fn main() {
    let postgresql_host = env_get("POSTGRESQL_ADDON_HOST");
    let postgresql_database = env_get("POSTGRESQL_ADDON_DB");
    let postgresql_username = env_get("POSTGRESQL_ADDON_USERNAME");
    let postgresql_password = env_get("POSTGRESQL_ADDON_PASSWORD");
    let postgresql_port = env_get_num("POSTGRESQL_ADDON_PORT", 5421);

    let connection_string =
        format!("postgresql://{postgresql_username}:{postgresql_password}@{postgresql_host}:{postgresql_port}/{postgresql_database}")
            .as_str();


    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(connection_string).await?;

    let app = routes::app();

    let listener =
        tokio::net::TcpListener::bind("localhost:8080").await?;

    axum::serve(listener, app).await?
}
