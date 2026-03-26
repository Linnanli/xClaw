use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Token error: {0}")]
    TokenError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("DLP error: {0}")]
    DlpError(String),
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
