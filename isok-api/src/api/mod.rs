mod docs;
mod errors;

use crate::config::ApiConfig;
use crate::db::DbHandler;
use crate::errors::Result;
use aide::Error;
use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use aide::openapi::{Info, OpenApi};
use axum::{Extension, Json};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::watch::Receiver;
use tokio::task::JoinHandle;
use tower_http::trace::TraceLayer;
use tracing::{error, info, trace};

pub(super) struct ApiStateInner {
    pub _db: DbHandler,
}

type ApiState = Arc<ApiStateInner>;

fn public_routes(_state: ApiState) -> ApiRouter {
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
        .nest("/docs", docs::router())
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
        .finish_api(&mut api)
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
