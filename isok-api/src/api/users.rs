use crate::api::ApiState;
use crate::api::errors::ApiResult;
use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, patch_with};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use isok_data::models::{PatchUser, User, UserView};

async fn get_me(
    State(_state): State<ApiState>,
    Extension(me): Extension<User>,
) -> ApiResult<Json<UserView>> {
    Ok(Json(me.into()))
}
async fn patch_me(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Json(patch): Json<PatchUser>,
) -> ApiResult<StatusCode> {
    if patch.email.is_none() && patch.password.is_none() {
        return Ok(StatusCode::NO_CONTENT);
    }

    state
        .db
        .users_patch_user(
            me.id,
            patch.email.map(|e| e.to_string()),
            match patch.password {
                Some(password) => Some(state.hasher.hash_password(&password)?),
                None => None,
            },
        )
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_me(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
) -> ApiResult<StatusCode> {
    state.db.users_delete(me.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/me",
            get_with(get_me, |op| {
                op.tag("users")
                    .id("getMeV1")
                    .description("Get current user")
            }),
        )
        .api_route(
            "/me",
            patch_with(patch_me, |op| {
                op.tag("users")
                    .id("editMeV1")
                    .description("Modify current user email/password")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/me",
            delete_with(delete_me, |op| {
                op.tag("users")
                    .id("deleteMeV1")
                    .description("Delete current user")
                    .response::<204, ()>()
            }),
        )
        .with_state(state)
}
