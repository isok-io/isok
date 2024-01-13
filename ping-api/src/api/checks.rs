use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use log::info;
use uuid::Uuid;

use ping_data::check::{CheckInput, CheckKind, CheckOutput};

use crate::api::ApiHandler;

pub async fn list_checks(
    State(handler): State<Arc<ApiHandler>>,
) -> Result<Json<Vec<CheckOutput>>, impl IntoResponse> {
    handler.db.get_checks().await.map(|c| {
        c.into_iter()
            .map(|c| c.into())
            .collect::<Vec<CheckOutput>>()
            .into()
    })
}

pub async fn get_check(
    State(handler): State<Arc<ApiHandler>>,
    Path(check_id): Path<Uuid>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    handler.db.get_check(check_id).await.map(|c| {
        let res: CheckOutput = c.into();
        res.into()
    })
}

pub async fn create_check(
    State(handler): State<Arc<ApiHandler>>,
    Json(check): Json<CheckInput>,
) -> Result<(), impl IntoResponse> {
    handler.db.insert_check(check).await
}

pub async fn change_check_kind(
    State(handler): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(check_kind): Json<CheckKind>,
) -> Result<(), impl IntoResponse> {
    handler.db.change_check_kind(id, check_kind).await
}

pub async fn change_check_interval(
    State(handler): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(interval): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    handler.db.change_check_interval(id, interval).await
}

pub async fn change_check_max_latency(
    State(handler): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
    Json(max_latency): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    handler.db.change_check_max_latency(id, max_latency).await
}

pub async fn delete_check(
    State(handler): State<Arc<ApiHandler>>,
    Path(id): Path<Uuid>,
) -> Result<(), impl IntoResponse> {
    handler.db.delete_check(id).await
}
