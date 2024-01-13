pub use std::fmt::Display;
use std::fmt::Formatter;

pub use argon2::password_hash;
pub use axum::body::Body;
pub use axum::response::{IntoResponse, Response};
pub use http::StatusCode;
pub use log::error;

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

pub struct NotFoundError<Model: Display, Value: Display> {
    pub model: Model,
    pub value: Value,
}

impl<Model: Display, Value: Display> IntoResponse for NotFoundError<Model, Value> {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::new(format!(
                "{} {} not found",
                self.model, self.value
            )))
            .unwrap()
    }
}

pub struct InvalidInput<Field: Display, Reason: Display> {
    pub field_name: Field,
    pub reason: Reason,
}

impl<Reason: Display> InvalidInput<&str, Reason> {
    pub fn new(reason: Reason) -> Self {
        Self {
            field_name: "",
            reason,
        }
    }
}

impl<Field: Display, Reason: Display> InvalidInput<Field, Reason> {
    pub fn with_field_name(self, field: Field) -> Self {
        Self {
            field_name: field,
            reason: self.reason,
        }
    }
}

impl<Field: Display, Reason: Display> IntoResponse for InvalidInput<Field, Reason> {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::new(format!(
                "invalid value for `{}`: {}",
                self.field_name, self.reason
            )))
            .unwrap()
    }
}

pub struct Forbidden;

impl IntoResponse for Forbidden {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::new("Forbidden".to_string()))
            .unwrap()
    }
}

pub struct PasswordHashError(pub password_hash::Error);

impl IntoResponse for PasswordHashError {
    fn into_response(self) -> Response {
        error!(target: "PASSWORD_HASH", "{}", self.0);

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::new("Internal server error".to_string()))
            .unwrap()
    }
}

pub struct WrongCredentials;

impl IntoResponse for WrongCredentials {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::new("Wrong login or password".to_string()))
            .unwrap()
    }
}

pub struct Unauthorized;

impl IntoResponse for Unauthorized {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::new("".to_string()))
            .unwrap()
    }
}

enum BiscuitErrors {
    Token(biscuit_auth::error::Token),
    Format(biscuit_auth::error::Format),
    ExtractUuid(uuid::Error),
    None,
}

impl Display for BiscuitErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BiscuitErrors::Token(e) => write!(f, "Token error: {:?}", e),
            BiscuitErrors::Format(e) => write!(f, "Format error: {:?}", e),
            BiscuitErrors::ExtractUuid(e) => write!(f, "Extract error: {:?}", e),
            BiscuitErrors::None => write!(f, "Biscuit error: "),
        }
    }
}

pub struct BiscuitError(BiscuitErrors);

impl BiscuitError {
    pub fn from_token(e: biscuit_auth::error::Token) -> Self {
        Self(BiscuitErrors::Token(e))
    }

    pub fn from_format(e: biscuit_auth::error::Format) -> Self {
        Self(BiscuitErrors::Format(e))
    }

    pub fn from_uuid(e: uuid::Error) -> Self {
        Self(BiscuitErrors::ExtractUuid(e))
    }

    pub fn none() -> Self {
        Self(BiscuitErrors::None)
    }
}

impl IntoResponse for BiscuitError {
    fn into_response(self) -> Response {
        error!(target: "BISCUIT", "{}", self.0);

        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::new("Internal server error".to_string()))
            .unwrap()
    }
}
