use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("User already exists")]
    UserExists,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error")]
    InternalError,
}

pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            Error::AuthFailed(_) => (StatusCode::UNAUTHORIZED, "Authentication failed"),
            Error::InvalidCredentials => (StatusCode::UNAUTHORIZED, "Invalid credentials"),
            Error::UserNotFound => (StatusCode::NOT_FOUND, "User not found"),
            Error::UserExists => (StatusCode::CONFLICT, "User already exists"),
            Error::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            Error::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired"),
            Error::Unauthorized => (StatusCode::FORBIDDEN, "Unauthorized"),
            Error::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.to_string()
        }));

        (status, body).into_response()
    }
}
