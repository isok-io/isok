use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use log::info;
use uuid::Uuid;

use ping_data::check::{CheckInput, CheckOutput};

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

pub async fn update_check() {
    todo!()
}

pub async fn delete_check() {
    todo!()
}
