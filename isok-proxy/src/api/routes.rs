use axum::body::Body;
use axum::response::Response;
pub use axum::routing::{delete, get, post, put};
pub use axum::{middleware, Router};

pub use crate::api::auth::login_handler;
use crate::api::checks::{change_check_interval, change_check_kind, change_check_max_latency};
pub use crate::api::checks::{create_check, delete_check, get_check, list_checks};
use crate::api::organizations::{
    add_member_to_organization, change_member_role_in_organization, delete_member_in_organization,
};
pub use crate::api::organizations::{
    create_organization, delete_organization, get_organization, list_organizations,
};
pub use crate::api::users::{
    change_user_email, change_user_password, create_user, delete_user, get_user, list_users,
    rename_user,
};
use crate::api::ServerState;

pub fn app(server_state: ServerState) -> Router<()> {
    Router::new()
        .route("/teapot", get(teapot))
        .route("/ping", get(ping))
        .merge(auth_routes(server_state.clone()))
        .nest(
            "/checks/:organization_id",
            checks_router(server_state.clone()),
        )
        .nest("/users", users_router(server_state.clone()))
        .nest("/organizations", organizations_router(server_state))
}

pub fn auth_routes(server_state: ServerState) -> Router<()> {
    Router::new()
        .route("/login", post(login_handler))
        .with_state(server_state)
}

pub fn users_router(server_state: ServerState) -> Router<()> {
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
                    server_state.clone(),
                    crate::api::middlewares::middleware,
                )),
        )
        .route("/", post(create_user))
        .with_state(server_state)
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
        .route_layer(middleware::from_fn_with_state(
            server_state.clone(),
            crate::api::middlewares::middleware,
        ))
        .with_state(server_state)
}

pub fn organizations_router(server_state: ServerState) -> Router<()> {
    Router::new()
        .route("/", get(list_organizations))
        .route("/:id", get(get_organization))
        .route("/", post(create_organization))
        .route("/:id", delete(delete_organization))
        .route("/:id/members", post(add_member_to_organization))
        .route(
            "/:id/members/:user_id",
            put(change_member_role_in_organization),
        )
        .route(
            "/:id/members/:user_id",
            delete(delete_member_in_organization),
        )
        .route_layer(middleware::from_fn_with_state(
            server_state.clone(),
            crate::api::middlewares::middleware,
        ))
        .with_state(server_state)
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
