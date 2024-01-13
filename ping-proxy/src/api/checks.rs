pub use std::sync::Arc;

pub use axum::extract::{Path, State};
pub use axum::response::IntoResponse;
pub use axum::Json;
pub use log::info;
pub use uuid::Uuid;

pub use ping_data::check::CheckOutput;

pub use crate::api::errors::NotFoundError;
pub use crate::api::ApiHandler;

pub async fn list_checks(State(state): State<Arc<ApiHandler>>) -> Json<Vec<CheckOutput>> {
    crate::utils::proxy::apis_req_vec(state, "checks")
        .await
        .into()
}

pub async fn get_check(
    State(state): State<Arc<ApiHandler>>,
    Path(check_id): Path<Uuid>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    match crate::utils::proxy::apis_req::<CheckOutput>(
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

pub async fn create_check() {
    todo!()
}

pub async fn update_check() {
    todo!()
}

pub async fn delete_check() {
    todo!()
}
