use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::error;

pub enum RequestError {
    Sqlx(sqlx::Error),
    NotFound { model: &'static str, value: String },
    InternalError,
}

impl IntoResponse for RequestError {
    fn into_response(self) -> Response {
        let internal_error = Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::new("Internal server error".to_string()))
            .unwrap();

        let not_found = |model: &'static str, value: String| {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::new(format!("{model} {value} not found")))
                .unwrap()
        };

        match self {
            RequestError::Sqlx(e) => {
                match e.as_database_error() {
                    Some(e) => error!(target: "DB", "{e}"),
                    None => error!(target: "DB", "unknown database error"),
                };
                internal_error
            }
            RequestError::NotFound { model, value } => not_found(model, value),
            RequestError::InternalError => internal_error,
        }
    }
}
