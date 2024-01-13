use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::error;

pub struct DbQueryError(pub sqlx::Error);

impl IntoResponse for DbQueryError {
    fn into_response(self) -> Response {
        match self.0.as_database_error() {
            Some(e) => error!(target: "DB", "{e}"),
            None => error!(target: "DB", "unknown database error"),
        }

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::new("Internal server error".to_string()))
            .unwrap()
    }
}
