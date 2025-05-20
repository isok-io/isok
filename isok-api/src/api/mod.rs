mod auth;
mod docs;
mod errors;
mod users;

pub use crate::api::auth::Hasher;
use crate::api::errors::ApiError;
use crate::config::ApiConfig;
use crate::db::DbHandler;
use crate::errors::Result;
use aide::Error;
use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::openapi::{Info, OpenApi, SecurityScheme};
use axum::body::Body;
use axum::extract::Request;
use axum::http::Response;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use biscuit_auth::{Authorizer, Biscuit, KeyPair};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch::Receiver;
use tower_http::trace::TraceLayer;
use tracing::{error, info, trace};
use uuid::Uuid;

pub(super) struct ApiStateInner {
    pub db: DbHandler,
    pub hasher: Hasher,
    pub keypair: KeyPair,
}

type ApiState = Arc<ApiStateInner>;

async fn auth_middleware(mut request: Request, next: Next, state: ApiState) -> Response<Body> {
    let Some(token) = token_extract(&mut request) else {
        return ApiError::unauthorized().into_response();
    };

    let Ok(biscuit) = Biscuit::from_base64(token, state.keypair.public()) else {
        return ApiError::unauthorized().into_response();
    };

    let mut authorizer = Authorizer::new();
    if authorizer
        .add_code(r#"allow if user($u);"#)
        .and_then(|_| authorizer.add_token(&biscuit))
        .and_then(|_| authorizer.authorize())
        .is_err()
    {
        return ApiError::unauthorized().into_response();
    };

    let Some((id,)) = authorizer
        .query::<&str, (String,), _>("data($id) <- user($id)")
        .ok()
        .and_then(|mut query| query.pop())
    else {
        return ApiError::unauthorized().into_response();
    };

    let Ok(id) = Uuid::parse_str(&id) else {
        return ApiError::internal(&format!("Can't parse uuid {id}")).into_response();
    };

    match state
        .db
        .users_get_by_id(id)
        .await
        .map_err(ApiError::from)
        .and_then(|user| user.ok_or(ApiError::unauthorized()))
    {
        Ok(user) => request.extensions_mut().insert(user),
        Err(error) => return error.into_response(),
    };

    next.run(request).await
}

fn public_routes(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/ping",
            get_with(
                || async { Json("PONG!") },
                |op| {
                    op.id("ping")
                        .description("Ping the api")
                        .response_with::<200, Json<String>, _>(|res| {
                            res.description("PONG!").example("PONG!")
                        })
                },
            ),
        )
        .nest_api_service("/v1", auth::router(state.clone()))
        .nest("/docs", docs::router())
}

fn auth_routes(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .nest_api_service("/v1/users", users::router(state.clone()))
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, state.clone())
        }))
        .with_path_items(|op| op.security_requirement("UserAuth"))
}

fn token_extract(request: &mut Request) -> Option<String> {
    request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|e| e.to_str().ok())
        .and_then(|authorization| {
            authorization
                .to_string()
                .strip_prefix("Bearer ")
                .map(ToString::to_string)
        })
}

pub async fn run(
    config: ApiConfig,
    api_state: ApiState,
    mut shutdown_rx: Receiver<()>,
) -> Result<()> {
    aide::generate::on_error(|error| {
        match error {
            Error::ResponseExists(_) => trace!(?error, "aide generate error"),
            _ => error!(?error, "aide generate error"),
        };
    });

    aide::generate::extract_schemas(true);
    aide::generate::all_error_responses(true);
    aide::generate::infer_responses(true);
    aide::generate::inferred_empty_response_status(204);

    let mut api = OpenApi {
        info: Info {
            title: "ISOK API".into(),
            description: Some("Openapi of isok api".into()),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let app = ApiRouter::new()
        .merge(public_routes(api_state.clone()))
        .merge(auth_routes(api_state.clone()))
        .finish_api_with(&mut api, |api| {
            api.security_scheme(
                "UserAuth",
                SecurityScheme::Http {
                    scheme: "bearer".to_string(),
                    bearer_format: Some("biscuit".into()),
                    description: Some("User token".into()),
                    extensions: Default::default(),
                },
            )
        })
        .layer(Extension(Arc::new(api)))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(config.addresses.as_slice()).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            info!(
                "Listening on {:#}",
                config
                    .addresses
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if let Err(error) = shutdown_rx.changed().await {
                error!(?error, "Cannot listen for shutdown signal");
            }
            info!("Shutting down the api");
        })
        .await?;
    info!("API stopped");
    Ok(())
}
