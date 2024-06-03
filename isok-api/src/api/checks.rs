use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use log::info;
use uuid::Uuid;

use isok_data::check::{CheckInput, CheckKind, CheckOutput};

use crate::api::ApiHandler;

use super::errors::RequestError;
use super::ApiHandlerState;

pub async fn list_checks(
    State(handler): State<ApiHandlerState>,
) -> Result<Json<Vec<CheckOutput>>, impl IntoResponse> {
    let handler = handler.lock().await;
    handler.db.get_checks().await.map(|c| {
        c.into_iter()
            .map(|c| c.into())
            .collect::<Vec<CheckOutput>>()
            .into()
    })
}

pub async fn get_check(
    State(handler): State<ApiHandlerState>,
    Path(check_id): Path<Uuid>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    let handler = handler.lock().await;
    handler.db.get_check(check_id).await.map(|c| {
        let res: CheckOutput = c.into();
        res.into()
    })
}

pub async fn create_check(
    State(handler): State<ApiHandlerState>,
    Json(check_input): Json<CheckInput>,
) -> Result<(), impl IntoResponse> {
    let mut handler = handler.lock().await;
    let check = handler.db.insert_check(check_input).await?;
    handler.pulsar_client.add_check(check).await;
    Ok::<(), RequestError>(())
}

pub async fn change_check_kind(
    State(handler): State<ApiHandlerState>,
    Path(id): Path<Uuid>,
    Json(check_kind): Json<CheckKind>,
) -> Result<(), impl IntoResponse> {
    let handler = handler.lock().await;
    handler.db.change_check_kind(id, check_kind).await
}

pub async fn change_check_interval(
    State(handler): State<ApiHandlerState>,
    Path(id): Path<Uuid>,
    Json(interval): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    let handler = handler.lock().await;
    handler.db.change_check_interval(id, interval).await
}

pub async fn change_check_max_latency(
    State(handler): State<ApiHandlerState>,
    Path(id): Path<Uuid>,
    Json(max_latency): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    let handler = handler.lock().await;

    handler.db.change_check_max_latency(id, max_latency).await
}

pub async fn delete_check(
    State(handler): State<ApiHandlerState>,
    Path(id): Path<Uuid>,
) -> Result<(), impl IntoResponse> {
    let mut handler = handler.lock().await;
    let check = handler.db.delete_check(id).await?;
    handler.pulsar_client.remove_check(check).await;
    Ok::<(), RequestError>(())
}
