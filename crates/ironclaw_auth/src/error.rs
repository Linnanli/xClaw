use thiserror::Error;

/// Authentication errors.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token")]
    InvalidToken,
}

/// Result type for authentication operations.
pub type Result<T> = std::result::Result<T, AuthError>;
