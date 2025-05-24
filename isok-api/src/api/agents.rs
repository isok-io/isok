use crate::api::errors::{ApiError, ApiResult};
use crate::api::{ApiState, token_extract};
use aide::axum::ApiRouter;
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{Response, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use isok_data::models::{AgentDetailsView, AgentInput, AgentView};

async fn get_agents(State(state): State<ApiState>) -> ApiResult<Json<Vec<AgentView>>> {
    let agents = state
        .db
        .agents_get_all(None)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(Json(agents))
}

async fn get_agent(
    State(state): State<ApiState>,
    Path(agent_id): Path<String>,
) -> ApiResult<Json<AgentDetailsView>> {
    let Some(agent) = state.db.agents_get_by_id(&agent_id).await? else {
        return Err(ApiError::not_found(format!("Agent {agent_id} not found")));
    };

    let checks = state.db.agents_get_checks(&agent_id).await?;

    Ok(Json(AgentDetailsView {
        view: agent.into(),
        checks,
    }))
}

async fn add_agent(
    State(state): State<ApiState>,
    Json(agent): Json<AgentInput>,
) -> ApiResult<StatusCode> {
    state.agents.add_agent(agent).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_agent(
    State(state): State<ApiState>,
    Json(agent_id): Json<String>,
) -> ApiResult<StatusCode> {
    state.agents.remove_agent(agent_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn auth_middleware(mut request: Request, next: Next, token: String) -> Response<Body> {
    match token_extract(&mut request) {
        Some(t) if t == token => next.run(request).await,
        _ => ApiError::unauthorized().into_response(),
    }
}

pub fn router(state: ApiState, token: String) -> ApiRouter {
    ApiRouter::new()
        .route("/", post(add_agent).get(get_agents).delete(delete_agent))
        .route("/{id}", get(get_agent))
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, token.clone())
        }))
        .with_state(state)
}
