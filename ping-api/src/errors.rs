use std::fmt::{Debug, Display, Formatter};
use axum::body::Body;
use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};

use serde::{Deserializer, Serialize, Serializer};
use serde_json::json;

#[derive(Serialize, Debug)]
pub enum Error {
    NotFound,
    RepositoryError(SQLError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(Body::empty()).unwrap(),
            Error::RepositoryError(err) => {
                let message = json!({
                    "message": err
                }).as_str().unwrap();
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "application/json")
                    .body(Body::from(message)).unwrap()
            }
        }
    }
}

pub struct SQLError(pub sqlx::Error);

impl Serialize for SQLError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(self.0.to_string().as_str())
    }
}

impl Debug for SQLError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for SQLError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl std::error::Error for SQLError {}