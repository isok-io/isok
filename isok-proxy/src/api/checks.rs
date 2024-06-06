use std::fmt::Formatter;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub use axum::extract::{Path, State};
pub use axum::response::IntoResponse;
use axum::Extension;
pub use axum::Json;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};
pub use uuid::Uuid;

pub use isok_data::check::CheckOutput;
use isok_data::check::{CheckInput, CheckKind};
use isok_data::owner::User;

pub use crate::api::errors::NotFoundError;
use crate::api::errors::Unauthorized;
use crate::api::ServerState;
use crate::DbHandler;

pub enum OrgPath {
    User,
    Org(Uuid),
}

impl OrgPath {
    pub async fn to_organization_id(
        self,
        user: &User,
        db_handler: Arc<DbHandler>,
    ) -> Result<Uuid, Unauthorized> {
        match self {
            OrgPath::User => Ok(user.self_organization.organization_id),
            OrgPath::Org(uuid) => {
                if uuid != user.self_organization.organization_id {
                    db_handler
                        .is_user_in_organization(&user.user_id, &uuid)
                        .await
                        .map_err(|_| Unauthorized)?;
                }
                Ok(uuid)
            }
        }
    }
}

struct OrgPathVisitor;

impl<'de> Visitor<'de> for OrgPathVisitor {
    type Value = OrgPath;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "Expecting valid organization path")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        match v {
            "me" => Ok(OrgPath::User),
            uuid => Ok(OrgPath::Org(Uuid::from_str(uuid).map_err(E::custom)?)),
        }
    }
}

impl<'de> Deserialize<'de> for OrgPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(OrgPathVisitor)
    }
}

pub async fn list_checks(
    State(state): State<ServerState>,
    Path(organization_id): Path<OrgPath>,
    Extension(current_user): Extension<User>,
) -> Result<Json<Vec<CheckOutput>>, impl IntoResponse> {
    let organization_id = organization_id
        .to_organization_id(&current_user, state.db)
        .await?;

    Ok::<Json<Vec<CheckOutput>>, Unauthorized>(
        crate::utils::proxy::get_all(state.apis, format!("checks/{organization_id}"))
            .await
            .into(),
    )
}

pub async fn get_check(
    State(state): State<ServerState>,
    Path((organization_id, check_id)): Path<(OrgPath, Uuid)>,
    Extension(current_user): Extension<User>,
) -> Result<Json<CheckOutput>, impl IntoResponse> {
    let organization_id = organization_id
        .to_organization_id(&current_user, state.db)
        .await
        .map_err(|e| e.into_response())?;

    match crate::utils::proxy::get_one::<_, CheckOutput>(
        state.apis,
        format!("checks/{organization_id}"),
        check_id.as_hyphenated().to_string(),
    )
    .await
    {
        Some(check) => Ok(check.into()),
        None => Err(NotFoundError {
            model: "check",
            value: check_id,
        }
        .into_response()),
    }
}

pub async fn create_check(
    State(state): State<ServerState>,
    Path(organization_id): Path<OrgPath>,
    Extension(current_user): Extension<User>,
    Json(check): Json<CheckInput>,
) -> impl IntoResponse {
    let organization_id = match organization_id
        .to_organization_id(&current_user, state.db)
        .await
    {
        Ok(uuid) => uuid,
        Err(e) => return e.into_response(),
    };

    crate::utils::proxy::create(
        state.apis,
        format!("checks/{organization_id}"),
        check.clone().region,
        check,
    )
    .await
    .into_response()
}

pub async fn change_check_kind(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(OrgPath, Uuid)>,
    Extension(current_user): Extension<User>,
    Json(check_kind): Json<CheckKind>,
) -> impl IntoResponse {
    let organization_id = match organization_id
        .to_organization_id(&current_user, state.db)
        .await
    {
        Ok(uuid) => uuid,
        Err(e) => return e.into_response(),
    };

    crate::utils::proxy::update(
        state.apis,
        format!("checks/{organization_id}/{id}/kind").as_str(),
        None,
        check_kind,
    )
    .await
    .into_response()
}

pub async fn change_check_interval(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(OrgPath, Uuid)>,
    Extension(current_user): Extension<User>,
    Json(interval): Json<Duration>,
) -> impl IntoResponse {
    let organization_id = match organization_id
        .to_organization_id(&current_user, state.db)
        .await
    {
        Ok(uuid) => uuid,
        Err(e) => return e.into_response(),
    };

    crate::utils::proxy::update(
        state.apis,
        format!("checks/{organization_id}/{id}/interval").as_str(),
        None,
        interval,
    )
    .await
    .into_response()
}

pub async fn change_check_max_latency(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(OrgPath, Uuid)>,
    Extension(current_user): Extension<User>,
    Json(max_latency): Json<Duration>,
) -> impl IntoResponse {
    let organization_id = match organization_id
        .to_organization_id(&current_user, state.db)
        .await
    {
        Ok(uuid) => uuid,
        Err(e) => return e.into_response(),
    };

    crate::utils::proxy::update(
        state.apis,
        format!("checks/{organization_id}/{id}/max_latency").as_str(),
        None,
        max_latency,
    )
    .await
    .into_response()
}

pub async fn delete_check(
    State(state): State<ServerState>,
    Path((organization_id, id)): Path<(OrgPath, Uuid)>,
    Extension(current_user): Extension<User>,
) -> impl IntoResponse {
    let organization_id = match organization_id
        .to_organization_id(&current_user, state.db)
        .await
    {
        Ok(uuid) => uuid,
        Err(e) => return e.into_response(),
    };

    crate::utils::proxy::delete(
        state.apis,
        format!("checks/{organization_id}"),
        id.as_hyphenated().to_string(),
    )
    .await
    .into_response()
}
