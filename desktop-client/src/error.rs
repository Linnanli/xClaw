use thiserror::Error;
use serde::Serialize;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Keychain error: {0}")]
    KeychainError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Session expired")]
    SessionExpired,

    #[error("Unauthorized")]
    Unauthorized,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
