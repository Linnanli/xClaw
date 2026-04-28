//! Error types for the network proxy.

use std::io;

/// Result alias for proxy operations.
pub type Result<T> = std::result::Result<T, ProxyError>;

/// Errors emitted by the network-proxy crate.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// Failed to bind the listener or get its local address.
    #[error("proxy bind failure: {reason}")]
    Bind { reason: String },

    /// Lower-level I/O error.
    #[error("proxy io error: {0}")]
    Io(#[from] io::Error),

    /// Generic proxy error (forwarding, decoding, ...).
    #[error("proxy error: {reason}")]
    Other { reason: String },
}

impl ProxyError {
    pub fn bind(reason: impl Into<String>) -> Self {
        Self::Bind {
            reason: reason.into(),
        }
    }

    pub fn other(reason: impl Into<String>) -> Self {
        Self::Other {
            reason: reason.into(),
        }
    }
}
