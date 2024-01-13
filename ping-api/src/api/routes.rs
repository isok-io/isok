use std::sync::Arc;

use axum::body::Body;
use axum::http::Response;
pub use axum::routing::{delete, get, post, put};
pub use axum::Router;

use super::checks::{create_check, get_check, list_checks};
use super::ApiHandler;

pub fn app(api_handler: Arc<ApiHandler>) -> Router<()> {
    Router::new()
        .route("/ping", get(ping))
        .route("/teapot", get(teapot))
        .nest("/checks", checks_router(api_handler))
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

pub fn checks_router(handler: Arc<ApiHandler>) -> Router<()> {
    Router::new()
        .route("/", get(list_checks))
        .route("/:id", get(get_check))
        .route("/", post(create_check))
        // .route("/:id", put(update_check))
        // .route("/:id", delete(delete_check))
        .with_state(handler)
}
