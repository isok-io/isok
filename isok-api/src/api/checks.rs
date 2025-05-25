use crate::api::ApiState;
use crate::api::errors::{ApiError, ApiResult};
use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, Json};
use isok_data::models::{
    ApiCheck, ApiCheckInput, ApiCheckMetrics, ApiChecksSummary, CHECK_SCHEMA_HTTP_V1, CheckSchema,
    User,
};
use schemars::JsonSchema;
use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use std::ops::Deref;
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
struct TenantPath {
    tenant: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct TenantCheckPath {
    tenant: Uuid,
    check_id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct MetricsFilter {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    points: usize,
}

async fn get_meta() -> Json<Vec<&'static CheckSchema>> {
    Json(vec![CHECK_SCHEMA_HTTP_V1.deref()])
}

async fn get_checks(
    State(state): State<ApiState>,
    Path(path): Path<TenantPath>,
) -> ApiResult<Json<Vec<ApiCheck>>> {
    let checks = state.db.checks_get_by_tenant(path.tenant).await?;
    Ok(Json(checks))
}

async fn create_check(
    State(state): State<ApiState>,
    Path(path): Path<TenantPath>,
    Json(check): Json<ApiCheckInput>,
) -> ApiResult<StatusCode> {
    let check = ApiCheck::from_input(check, Uuid::new_v4(), path.tenant);
    state.db.checks_insert_check(&check).await?;
    state.agents.add_check(check).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_check(
    State(state): State<ApiState>,
    Path(path): Path<TenantCheckPath>,
) -> ApiResult<Json<ApiCheck>> {
    let check = state
        .db
        .checks_get_by_ids(vec![path.check_id])
        .await?
        .into_iter()
        .find(|c| c.tenant == path.tenant)
        .ok_or(ApiError::not_found(format!(
            "check {} not found",
            path.check_id
        )))?;

    Ok(Json(check))
}

async fn get_check_metrics(
    State(_state): State<ApiState>,
    Extension(_me): Extension<User>,
    Path(_path): Path<TenantCheckPath>,
    Query(filter): Query<MetricsFilter>,
) -> ApiResult<Json<ApiCheckMetrics>> {
    Ok(Json((0..filter.points).map(|_| None).collect()))
}

async fn update_check(
    State(state): State<ApiState>,
    Path(path): Path<TenantCheckPath>,
    Json(check): Json<ApiCheckInput>,
) -> ApiResult<StatusCode> {
    state.agents.remove_check(path.check_id).await?;
    state.db.checks_delete_by_id(path.check_id).await?;

    let check = ApiCheck::from_input(check, path.check_id, path.tenant);
    state.db.checks_insert_check(&check).await?;
    state.agents.add_check(check).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_check(
    State(state): State<ApiState>,
    Path(path): Path<TenantCheckPath>,
) -> ApiResult<StatusCode> {
    state.agents.remove_check(path.check_id).await?;
    state.db.checks_delete_by_id(path.check_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_summary(
    State(state): State<ApiState>,
    Path(path): Path<TenantPath>,
    Query(filter): Query<MetricsFilter>,
) -> ApiResult<Json<ApiChecksSummary>> {
    let summary = state
        .db
        .checks_get_by_tenant(path.tenant)
        .await?
        .into_iter()
        .map(|c| (c.inner.id, (0..filter.points).map(|_| None).collect()))
        .collect();

    Ok(Json(summary))
}

async fn live(
    State(_state): State<ApiState>,
    Extension(_me): Extension<User>,
    Path(_path): Path<TenantPath>,
) -> ApiResult<()> {
    todo!()
}

#[derive(Deserialize)]
struct TenantCheckLayerPath {
    tenant: Uuid,
    check_id: Option<Uuid>,
}

async fn tenant_check(
    state: ApiState,
    Path(path): Path<TenantCheckLayerPath>,
    Extension(me): Extension<User>,
    request: Request,
    next: Next,
) -> ApiResult<Response> {
    if me.id != path.tenant && !state.db.orgs_is_user_in(path.tenant, me.id).await? {
        Err(ApiError::not_found(format!(
            "tenant {} not found",
            path.tenant
        )))?;
    }

    if let Some(check) = path.check_id {
        if !state.db.checks_is_tenant(check, path.tenant).await? {
            Err(ApiError::not_found(format!("check {check} not found")))?;
        }
    }

    let response = next.run(request).await;
    Ok(response)
}

pub fn public_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/meta",
        get_with(get_meta, |op| {
            op.tag("checks")
                .id("getChecksMetaV1")
                .description("Get checks' schema")
                .response_with::<200, Json<Vec<&CheckSchema>>, _>(|r| {
                    r.example(vec![CHECK_SCHEMA_HTTP_V1.deref()])
                })
        }),
    )
}

pub fn auth_router(state: ApiState) -> ApiRouter {
    let mstate = state.clone();
    ApiRouter::new()
        .api_route(
            "/",
            get_with(get_checks, |op| {
                op.tag("checks")
                    .id("getChecksV1")
                    .description("Get tenant's checks")
            })
            .post_with(create_check, |op| {
                op.tag("checks")
                    .id("createCheckV1")
                    .description("Create check")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/{check_id}",
            get_with(get_check, |op| {
                op.tag("checks").id("getCheckV1").description("Get check")
            })
            .put_with(update_check, |op| {
                op.tag("checks")
                    .id("updateCheckV1")
                    .description("Update check")
                    .response::<204, ()>()
            })
            .delete_with(delete_check, |op| {
                op.tag("checks")
                    .id("deleteCheckV1")
                    .description("Delete check")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/{check_id}/metrics",
            get_with(get_check_metrics, |op| {
                op.tag("checks")
                    .id("getCheckMetricsV1")
                    .description("Get check's metrics")
            }),
        )
        .api_route(
            "/summary",
            get_with(get_summary, |op| {
                op.tag("checks")
                    .id("getChecksSummaryV1")
                    .description("Get tenant's checks summary")
            }),
        )
        .api_route(
            "/live",
            get_with(live, |op| {
                op.tag("checks")
                    .id("liveChecksV1")
                    .description("Get tenant's checks updates")
            }),
        )
        .route_layer(axum::middleware::from_fn(
            move |me: Extension<User>,
                  path: Path<TenantCheckLayerPath>,
                  request: Request,
                  next: Next| { tenant_check(mstate.clone(), path, me, request, next) },
        ))
        .with_state(state)
}
