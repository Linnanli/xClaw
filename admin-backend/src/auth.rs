// Re-export from shared ironclaw_auth crate
pub use ironclaw_auth::{AuthManager, TokenClaims};

// Map ironclaw_auth errors to admin-backend errors
use crate::error::Error;

impl From<ironclaw_auth::AuthError> for Error {
    fn from(err: ironclaw_auth::AuthError) -> Self {
        match err {
            ironclaw_auth::AuthError::AuthFailed(msg) => Error::AuthFailed(msg),
            ironclaw_auth::AuthError::InvalidCredentials => Error::InvalidCredentials,
            ironclaw_auth::AuthError::TokenExpired => Error::TokenExpired,
            ironclaw_auth::AuthError::InvalidToken => Error::InvalidToken,
        }
    }
}
