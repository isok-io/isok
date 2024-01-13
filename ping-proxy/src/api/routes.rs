pub use std::sync::Arc;

use axum::body::Body;
use axum::response::Response;
pub use axum::routing::{delete, get, post, put};
pub use axum::{middleware, Router};

pub use crate::api::auth::login_handler;
pub use crate::api::checks::{create_check, delete_check, get_check, list_checks, update_check};
pub use crate::api::user::{
    change_user_email, change_user_password, create_user, delete_user, get_user, list_users,
    rename_user,
};
pub use crate::api::{ApiHandler, AuthHandler};

pub fn app(api_handler: Arc<ApiHandler>, auth_handler: Arc<AuthHandler>) -> Router<()> {
    Router::new()
        .route("/teapot", get(teapot))
        .route("/ping", get(ping))
        .merge(auth_routes(auth_handler.clone()))
        .nest("/checks", checks_router(api_handler, auth_handler.clone()))
        .nest("/users", users_router(auth_handler))
}

pub fn auth_routes(auth_handler: Arc<AuthHandler>) -> Router<()> {
    Router::new().route(
        "/login",
        post({
            let state = Arc::clone(&auth_handler);
            move |body| login_handler(body, state)
        }),
    )
}

pub fn users_router(auth_handler: Arc<AuthHandler>) -> Router<()> {
    Router::new()
        .merge(
            Router::new()
                .route("/", get(list_users))
                .route("/:id", get(get_user))
                .route("/:id/rename", put(rename_user))
                .route("/:id/email", put(change_user_email))
                .route("/:id/password", put(change_user_password))
                .route("/:id", delete(delete_user))
                .route_layer(middleware::from_fn_with_state(
                    auth_handler.clone(),
                    crate::api::middlewares::middleware,
                )),
        )
        .route("/", post(create_user))
        .with_state(auth_handler)
}
pub fn checks_router(api_handler: Arc<ApiHandler>, auth_handler: Arc<AuthHandler>) -> Router<()> {
    Router::new()
        .route("/", get(list_checks))
        .route("/:id", get(get_check))
        .route("/", post(create_check))
        .route("/:id", put(update_check))
        .route("/:id", delete(delete_check))
        .route_layer(middleware::from_fn_with_state(
            auth_handler,
            crate::api::middlewares::middleware,
        ))
        .with_state(api_handler)
}

async fn teapot() -> Response<Body> {
    Response::builder()
        .status(418)
        .body(Body::new("I am a teapot !".to_string()))
        .unwrap()
}

async fn ping() -> &'static str {
    "PONG !"
}
