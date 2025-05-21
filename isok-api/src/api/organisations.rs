use crate::api::ApiState;
use crate::api::errors::{ApiError, ApiResult};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete_with, get_with, post_with};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use isok_data::models::{
    Email, Organisation, OrganisationInput, OrganisationNameInput, OrganisationSimpleView,
    OrganisationView, RefinementOps, User,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
struct OrganisationPath {
    organisation_id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct OrganisationMemberPath {
    organisation_id: Uuid,
    user_id: Uuid,
}

async fn list_organisations(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
) -> ApiResult<Json<Vec<OrganisationSimpleView>>> {
    let orgs = state
        .db
        .orgs_get_user_orgs(me.id)
        .await?
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>();

    Ok(Json(orgs))
}
async fn create_organisation(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Json(org): Json<OrganisationInput>,
) -> ApiResult<Json<Uuid>> {
    state
        .db
        .orgs_insert_org(Organisation {
            id: Uuid::new_v4(),
            name: org.name.to_string(),
            members: vec![me.id],
            tags: Default::default(),
        })
        .await
        .map(Json)
        .map_err(From::from)
}

async fn get_organisation(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Path(path): Path<OrganisationPath>,
) -> ApiResult<Json<OrganisationView>> {
    let org = state.db.orgs_get_by_id(path.organisation_id).await;

    match org {
        Ok(Some(organisation)) if organisation.members.contains(&me.id) => {
            let mut members = Vec::with_capacity(organisation.members.len());
            for member in organisation.members {
                let member = state.db.users_get_by_id(member).await?.ok_or_else(|| {
                    ApiError::internal(format!("User {member} not found").as_str())
                })?;
                members.push(member.into());
            }

            Ok(Json(OrganisationView {
                id: organisation.id,
                name: organisation.name,
                members,
                tags: organisation.tags,
            }))
        }
        Ok(_) => Err(ApiError::not_found(format!(
            "Organisation {} not found",
            path.organisation_id
        ))),
        Err(error) => Err(error.into()),
    }
}

async fn update_organisation_name(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Path(path): Path<OrganisationPath>,
    Json(name): Json<OrganisationNameInput>,
) -> ApiResult<StatusCode> {
    let org = state.db.orgs_get_by_id(path.organisation_id).await;

    match org {
        Ok(Some(organisation)) if organisation.members.contains(&me.id) => state
            .db
            .orgs_update_name(path.organisation_id, &name.name)
            .await
            .map(|_| StatusCode::NO_CONTENT)
            .map_err(From::from),
        Ok(_) => Err(ApiError::not_found(format!(
            "Organisation {} not found",
            path.organisation_id
        ))),
        Err(error) => Err(error.into()),
    }
}

async fn add_member(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Path(path): Path<OrganisationPath>,
    Json(user): Json<String>,
) -> ApiResult<StatusCode> {
    let user =
        Email::refine(user).map_err(|_| ApiError::precondition_failed("Invalid email".into()))?;

    let org = state.db.orgs_get_by_id(path.organisation_id).await;

    match org {
        Ok(Some(organisation)) if organisation.members.contains(&me.id) => {
            let Some(user) = state.db.users_get_by_email(user.deref()).await? else {
                return Err(ApiError::bad_request(format!("User {user} not found")));
            };

            state
                .db
                .orgs_add_member(path.organisation_id, user.id)
                .await
                .map(|_| StatusCode::NO_CONTENT)
                .map_err(From::from)
        }
        Ok(_) => Err(ApiError::not_found(format!(
            "Organisation {} not found",
            path.organisation_id
        ))),
        Err(error) => Err(error.into()),
    }
}

async fn remove_member(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Path(path): Path<OrganisationMemberPath>,
) -> ApiResult<StatusCode> {
    let org = state.db.orgs_get_by_id(path.organisation_id).await;

    match org {
        Ok(Some(organisation)) if organisation.members.contains(&me.id) => state
            .db
            .orgs_remove_member(path.organisation_id, path.user_id)
            .await
            .map(|_| StatusCode::NO_CONTENT)
            .map_err(From::from),
        Ok(_) => Err(ApiError::not_found(format!(
            "Organisation {} not found",
            path.organisation_id
        ))),
        Err(error) => Err(error.into()),
    }
}

async fn delete_organisation(
    State(state): State<ApiState>,
    Extension(me): Extension<User>,
    Path(path): Path<OrganisationPath>,
) -> ApiResult<StatusCode> {
    let org = state.db.orgs_get_by_id(path.organisation_id).await;

    match org {
        Ok(Some(organisation)) if organisation.members.contains(&me.id) => state
            .db
            .orgs_delete(path.organisation_id)
            .await
            .map(|_| StatusCode::NO_CONTENT)
            .map_err(From::from),
        Ok(_) => Err(ApiError::not_found(format!(
            "Organisation {} not found",
            path.organisation_id
        ))),
        Err(error) => Err(error.into()),
    }
}

pub fn router(state: ApiState) -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(list_organisations, |op| {
                op.tag("organisations")
                    .id("listOrgsV1")
                    .description("List current user's organisations")
            }),
        )
        .api_route(
            "/",
            post_with(create_organisation, |op| {
                op.tag("organisations")
                    .id("createOrgV1")
                    .description("Create a new organisation")
            }),
        )
        .api_route(
            "/{organisation_id}",
            get_with(get_organisation, |op| {
                op.tag("organisations")
                    .id("getOrgV1")
                    .description("Get an organisation by id")
            })
            .put_with(update_organisation_name, |op| {
                op.tag("organisations")
                    .id("renameOrgV1")
                    .description("Update an organisation name")
                    .response::<204, ()>()
            })
            .delete_with(delete_organisation, |op| {
                op.tag("organisations")
                    .id("deleteOrgV1")
                    .description("Delete a organisation")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/{organisation_id}/members",
            post_with(add_member, |op| {
                op.tag("organisations")
                    .id("addOrgMemberV1")
                    .description("Add a member to an organisation")
                    .response::<204, ()>()
            }),
        )
        .api_route(
            "/{organisation_id}/members/{user_id}",
            delete_with(remove_member, |op| {
                op.tag("organisations")
                    .id("removeOrgMemberV1")
                    .description("Remove a member from an organisation")
                    .response::<204, ()>()
            }),
        )
        .with_state(state)
}
