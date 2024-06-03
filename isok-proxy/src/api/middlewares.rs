pub use std::sync::Arc;

pub use axum::extract::{Request, State};
pub use axum::middleware::Next;
pub use axum::response::{IntoResponse, Response};

pub use crate::api::errors::Unauthorized;
pub use crate::api::AuthHandler;
pub use crate::utils::auth::token::get_user_id_from_token;

pub async fn middleware(
    State(state): State<Arc<AuthHandler>>,
    mut request: Request,
    next: Next,
) -> Response {
    let user_id = match request
        .headers()
        .get("Authorization")
        .and_then(|e| e.to_str().ok())
        .and_then(|authorization| {
            authorization
                .to_string()
                .strip_prefix("Bearer ")
                .map(ToString::to_string)
        }) {
        None => return Unauthorized.into_response(),
        Some(bearer) => get_user_id_from_token(bearer, &state.private_key),
    };

    let user_id = match user_id {
        Ok(user_id) => user_id,
        Err(e) => return e.into_response(),
    };

    let user = match state.db.get_user(user_id).await {
        Ok(user) => user,
        Err(e) => {
            let _ = e.into_response();
            return Unauthorized.into_response();
        }
    };

    request.extensions_mut().insert(user);

    next.run(request).await
}
