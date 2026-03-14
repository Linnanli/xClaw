use thiserror::Error;

#[derive(Error, Debug)]
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
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Session expired")]
    SessionExpired,

    #[error("Unauthorized")]
    Unauthorized,
}

pub type Result<T> = std::result::Result<T, Error>;
