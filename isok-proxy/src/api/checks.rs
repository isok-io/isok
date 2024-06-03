pub use std::sync::Arc;
use std::time::Duration;

pub use axum::extract::{Path, State};
pub use axum::response::IntoResponse;
pub use axum::Json;
pub use log::info;
use isok_data::check::{CheckInput, CheckKind};
pub use uuid::Uuid;

pub use isok_data::check::CheckOutput;

pub use crate::api::errors::NotFoundError;
pub use crate::api::ApiHandler;

pub async fn list_checks(State(state): State<Arc<ApiHandler>>) -> Json<Vec<CheckOutput>> {
    crate::utils::proxy::get_all(state, "checks").await.into()
}

pub async fn get_check(
    State(state): State<Arc<ApiHandler>>,
    Path(check_id): Path<Uuid>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    match crate::utils::proxy::get_one::<CheckOutput>(
        state,
        "checks",
        check_id.as_hyphenated().to_string(),
    )
    .await
    {
        Some(check) => Ok(check.into()),
        None => Err(NotFoundError {
            model: "check",
            value: check_id,
        }),
    }
}

pub async fn create_check(
    State(state): State<Arc<ApiHandler>>,
    Json(check): Json<CheckInput>,
) -> impl IntoResponse {
    crate::utils::proxy::create(state, "checks", check.clone().region, check).await
}

pub async fn change_check_kind(
    State(state): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(check_kind): Json<CheckKind>,
) -> impl IntoResponse {
    crate::utils::proxy::update(
        state,
        format!("checks/{id}/kind").as_str(),
        None,
        check_kind,
    )
    .await
}

pub async fn change_check_interval(
    State(state): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(interval): Json<Duration>,
) -> impl IntoResponse {
    crate::utils::proxy::update(
        state,
        format!("checks/{id}/interval").as_str(),
        None,
        interval,
    )
    .await
}

pub async fn change_check_max_latency(
    State(state): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(max_latency): Json<Duration>,
) -> impl IntoResponse {
    crate::utils::proxy::update(
        state,
        format!("checks/{id}/max_latency").as_str(),
        None,
        max_latency,
    )
    .await
}

pub async fn delete_check(
    State(state): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    crate::utils::proxy::delete(state, "checks", id.as_hyphenated().to_string()).await
}
