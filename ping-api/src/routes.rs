use axum::Router;
use axum::routing::{delete, get, post, put};
use crate::auth::api::*;
use crate::checks::api::*;
use crate::users::api::*;

pub fn app() -> Router<()> {
    Router::new()
        .route("/", get(root_handler))
        .merge(auth_routes())
        .nest("/users", users_router())
        .nest("/checks", checks_router())
}

pub fn auth_routes() -> Router<()> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/logout", get(logout_handler))
}

pub fn users_router() -> Router<()> {
    Router::new()
        .route("/", get(list_users))
        .route("/:id", get(get_user))
        .route("/", post(create_user))
        .route("/:id", put(update_user))
        .route("/:id", delete(delete_user))
}

pub fn checks_router() -> Router<()> {
    Router::new()
        .route("/", get(list_checks))
        .route("/:id", get(get_check))
        .route("/", post(create_check))
        .route("/:id", put(update_check))
        .route("/:id", delete(delete_check))
}

async fn root_handler() -> &'static str {
    "Hello, world!"
}