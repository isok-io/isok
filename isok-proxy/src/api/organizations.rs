pub use axum::extract::{Path, State};
pub use axum::response::IntoResponse;
pub use axum::{Extension, Json};
use http::StatusCode;
pub use serde::{Deserialize, Serialize};
use sqlx::Error;
pub use uuid::Uuid;

pub use isok_data::owner::User;
use isok_data::owner::{
    NormalOrganization, Organization, OrganizationInput, OrganizationOutput, OrganizationType,
    OrganizationUserRole,
};

use crate::api::errors::DbQueryError;
pub use crate::api::errors::{Forbidden, NotFoundError};
use crate::api::ServerState;
use crate::utils::validator::valid_string_with_max_len;

pub async fn list_organizations(
    State(state): State<ServerState>,
    Extension(current_user): Extension<User>,
) -> Result<Json<Vec<OrganizationOutput>>, impl IntoResponse> {
    state
        .db
        .get_user_organizations(current_user.user_id)
        .await
        .map(|orgs| {
            orgs.into_iter()
                .map(|org| org.into())
                .collect::<Vec<OrganizationOutput>>()
                .into()
        })
}

pub async fn get_organization(
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Extension(current_user): Extension<User>,
) -> Result<Json<OrganizationOutput>, impl IntoResponse> {
    match state
        .db
        .get_user_organization(current_user.user_id, id)
        .await
    {
        Ok(org) => Ok(<Organization as Into<OrganizationOutput>>::into(org).into()),
        Err(DbQueryError(Error::RowNotFound)) => Err(NotFoundError {
            model: "organization",
            value: id,
        }
        .into_response()),
        Err(e) => Err(e.into_response()),
    }
}

pub async fn create_organization(
    State(state): State<ServerState>,
    Extension(current_user): Extension<User>,
    Json(organization): Json<OrganizationInput>,
) -> Result<(), impl IntoResponse> {
    if let Err(e) = valid_string_with_max_len(&organization.name, 256) {
        return Err(e.into_response());
    }

    state
        .db
        .insert_organization(current_user.user_id, organization)
        .await
        .map_err(|e| e.into_response())
}

pub async fn delete_organization(
    State(state): State<ServerState>,
    Path(id): Path<Uuid>,
    Extension(current_user): Extension<User>,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let organization = state
        .db
        .get_user_organization(current_user.user_id, id)
        .await
        .map_err(|e| match e {
            DbQueryError(Error::RowNotFound) => NotFoundError {
                model: "organization",
                value: id,
            }
            .into_response(),
            e => e.into_response(),
        })?;

    if let OrganizationType::NormalOrganization(NormalOrganization { users, .. }) =
        organization.organization_type
    {
        if users
            .iter()
            .filter(|u| {
                u.user.user_id == current_user.user_id && u.role == OrganizationUserRole::Owner
            })
            .last()
            .is_none()
        {
            return Err(Forbidden.into_response());
        }
    }

    state
        .db
        .delete_organization(id)
        .await
        .map(|_| StatusCode::NO_CONTENT.into_response())
        .map_err(|e| e.into_response())
}
