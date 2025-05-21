use crate::api::ApiState;
use crate::api::errors::ApiResult;
use aide::axum::ApiRouter;
use aide::axum::routing::get_with;
use axum::Json;
use axum::extract::State;
use isok_data::models::Region;

async fn get_regions(State(state): State<ApiState>) -> ApiResult<Json<Vec<Region>>> {
    state
        .db
        .regions_get_all()
        .await
        .map(Json)
        .map_err(Into::into)
}

pub fn router(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(get_regions, |op| {
                op.tag("regions")
                    .id("getRegionsV1")
                    .description("Get regions")
            }),
        )
        .with_state(state)
}
