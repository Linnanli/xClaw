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

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error")]
    InternalError,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Department has users")]
    DepartmentHasUsers,

    #[error("Department not found")]
    DepartmentNotFound,

    #[error("Skill not found")]
    SkillNotFound,

    #[error("Plugin not found")]
    PluginNotFound,

    #[error("Client not found")]
    ClientNotFound,

    #[error("账户已锁定，请稍后再试")]
    AccountLocked,
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
            Error::Conflict(_) => (StatusCode::CONFLICT, "Resource conflict"),
            Error::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            Error::TokenExpired => (StatusCode::UNAUTHORIZED, "Token expired"),
            Error::Unauthorized => (StatusCode::FORBIDDEN, "Unauthorized"),
            Error::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Error::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            Error::NotFound(_) => (StatusCode::NOT_FOUND, "Resource not found"),
            Error::Validation(_) => (StatusCode::BAD_REQUEST, "Validation error"),
            Error::DepartmentHasUsers => (StatusCode::BAD_REQUEST, "该部门下还有用户，无法删除"),
            Error::DepartmentNotFound => (StatusCode::NOT_FOUND, "Department not found"),
            Error::SkillNotFound => (StatusCode::NOT_FOUND, "Skill not found"),
            Error::PluginNotFound => (StatusCode::NOT_FOUND, "Plugin not found"),
            Error::ClientNotFound => (StatusCode::NOT_FOUND, "Client not found"),
            Error::AccountLocked => (StatusCode::LOCKED, "账户已锁定，请稍后再试"),
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.to_string()
        }));

        (status, body).into_response()
    }
}
