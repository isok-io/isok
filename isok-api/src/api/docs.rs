use aide::axum::ApiRouter;
use aide::openapi::OpenApi;
use aide::redoc::Redoc;
use aide::swagger::Swagger;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json};
use std::sync::Arc;

async fn openapi(Extension(api): Extension<Arc<OpenApi>>) -> impl IntoResponse {
    Json(api).into_response()
}

pub fn router() -> ApiRouter {
    ApiRouter::new()
        .route("/openapi.json", get(openapi))
        .route("/redoc", Redoc::new("/docs/openapi.json").axum_route())
        .route("/swagger", Swagger::new("/docs/openapi.json").axum_route())
}
