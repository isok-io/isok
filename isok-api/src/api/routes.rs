use axum::body::Body;
use axum::http::Response;
pub use axum::routing::{delete, get, post, put};
pub use axum::Router;

use super::checks::{
    change_check_interval, change_check_kind, change_check_max_latency, create_check, delete_check,
    get_check, list_checks,
};
use super::ServerState;

pub fn app(server_state: ServerState) -> Router<()> {
    Router::new()
        .route("/ping", get(ping))
        .route("/teapot", get(teapot))
        .nest("/checks/:organization_id", checks_router(server_state))
}

pub async fn ping() -> &'static str {
    "PONG !"
}

pub async fn teapot() -> Response<Body> {
    Response::builder()
        .status(418)
        .body(Body::new("I am a teapot !".to_string()))
        .unwrap()
}

pub fn checks_router(server_state: ServerState) -> Router<()> {
    Router::new()
        .route("/", get(list_checks))
        .route("/:id", get(get_check))
        .route("/", post(create_check))
        .route("/:id/kind", put(change_check_kind))
        .route("/:id/interval", put(change_check_interval))
        .route("/:id/max_latency", put(change_check_max_latency))
        .route("/:id", delete(delete_check))
        .with_state(server_state)
}
