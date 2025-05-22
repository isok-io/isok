use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{any, get},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::error;
use uuid::Uuid;

use crate::state::AgentState;
use isok_data::models::Check;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    INTERNAL_SERVER_ERROR,
    UNDELETABLE_CHECKS,
}

impl From<ErrorCode> for StatusCode {
    fn from(value: ErrorCode) -> Self {
        match value {
            ErrorCode::INTERNAL_SERVER_ERROR => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::UNDELETABLE_CHECKS => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub error: String,
    pub error_code: ErrorCode,
}

impl From<ApiError> for ApiErrorBody {
    fn from(value: ApiError) -> Self {
        ApiErrorBody {
            error: value.error,
            error_code: value.error_code,
        }
    }
}

pub struct ApiError {
    pub error: String,
    pub error_code: ErrorCode,
}

impl ApiError {
    pub fn internal(internal_msg: &str) -> Self {
        error!(internal_msg);
        ApiError {
            error: "Internal server error".to_string(),
            error_code: ErrorCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn undeletable_checks(check_ids: impl Iterator<Item = Uuid>) -> Self {
        let check_ids = check_ids
            .map(|e| format!("\"{e}\""))
            .reduce(|acc, e| acc + "," + &e)
            .unwrap_or_default();

        ApiError {
            error: format!("[{}]", check_ids),
            error_code: ErrorCode::UNDELETABLE_CHECKS,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status: StatusCode = self.error_code.into();
        let body: ApiErrorBody = self.into();
        (status, Json(body)).into_response()
    }
}

pub fn api(state: Arc<RwLock<AgentState>>) -> Router {
    Router::new()
        .route("/ping", any(ping))
        .route("/teapot", any(teapot))
        .nest("/checks", checks_api(state))
}

pub fn checks_api(state: Arc<RwLock<AgentState>>) -> Router {
    Router::new()
        .route("/", get(get_checks).post(post_checks).delete(delete_checks))
        .with_state(state)
}

pub async fn ping() -> &'static str {
    "PONG!"
}

pub async fn teapot() -> Response<Body> {
    Response::builder()
        .status(418)
        .body(Body::new("I am a teapot!".to_string()))
        .unwrap()
}

async fn get_checks(
    State(state): State<Arc<RwLock<AgentState>>>,
) -> Result<Json<Vec<Uuid>>, ApiError> {
    Ok(Json(
        state.read().await.checks.keys().map(Clone::clone).collect(),
    ))
}

async fn post_checks(
    State(state): State<Arc<RwLock<AgentState>>>,
    Json(input): Json<Vec<Check>>,
) -> Result<(), ApiError> {
    let mut state = state.write().await;
    for c in input {
        state.insert_check(c).await;
    }
    Ok(())
}

async fn delete_checks(
    State(state): State<Arc<RwLock<AgentState>>>,
    Json(input): Json<Vec<Uuid>>,
) -> Result<(), ApiError> {
    let mut state = state.write().await;
    let mut undeleted_checks = Vec::with_capacity(0);
    for c in input {
        if !state.delete_check(c).await {
            undeleted_checks.push(c);
        }
    }

    if undeleted_checks.is_empty() {
        Ok(())
    } else {
        Err(ApiError::undeletable_checks(undeleted_checks.into_iter()))
    }
}
