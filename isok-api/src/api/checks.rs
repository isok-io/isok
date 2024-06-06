use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::Json;
use uuid::Uuid;

use isok_data::check::{CheckInput, CheckKind, CheckOutput};

use super::errors::RequestError;
use super::ServerState;

pub async fn list_checks(
    State(state): State<ServerState>,
    Path(organization_id): Path<Uuid>,
) -> Result<Json<Vec<CheckOutput>>, impl IntoResponse> {
    state.db.get_checks().await.map(|c| {
        c.into_iter()
            .filter(|c| c.owner_id == organization_id)
            .map(|c| c.into())
            .collect::<Vec<CheckOutput>>()
            .into()
    })
}

pub async fn get_check(
    State(state): State<ServerState>,
    Path((organization_id, check_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    state.db.get_check(check_id).await.map(|c| {
        if c.owner_id != organization_id {
            Err(RequestError::NotFound {
                model: "check",
                value: c.check_id.as_hyphenated().to_string(),
            })
        } else {
            let res: CheckOutput = c.into();
            Ok(res.into())
        }
    })?
}

pub async fn create_check(
    State(state): State<ServerState>,
    Path(organization_id): Path<Uuid>,
    Json(check_input): Json<CheckInput>,
) -> Result<(), impl IntoResponse> {
    if check_input.owner_id != organization_id {
        return Err(RequestError::InternalError);
    }
    let mut pulsar_client = state.pulsar_client.lock().await;
    let check = state.db.insert_check(check_input).await?;
    pulsar_client.add_check(check).await;
    Ok::<(), RequestError>(())
}

pub async fn change_check_kind(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(Uuid, Uuid)>,
    Json(check_kind): Json<CheckKind>,
) -> Result<(), impl IntoResponse> {
    state
        .db
        .change_check_kind(id, check_kind, organization_id)
        .await
}

pub async fn change_check_interval(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(Uuid, Uuid)>,
    Json(interval): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    state
        .db
        .change_check_interval(id, interval, organization_id)
        .await
}

pub async fn change_check_max_latency(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(Uuid, Uuid)>,
    Json(max_latency): Json<Duration>,
) -> Result<(), impl IntoResponse> {
    state
        .db
        .change_check_max_latency(id, max_latency, organization_id)
        .await
}

pub async fn delete_check(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(Uuid, Uuid)>,
) -> Result<(), impl IntoResponse> {
    let mut pulsar_client = state.pulsar_client.lock().await;
    let check = state.db.delete_check(id, organization_id).await?;
    pulsar_client.remove_check(check).await;
    Ok::<(), RequestError>(())
}
