//! Errors produced by the [`SessionStore`](crate::store::SessionStore)
//! trait and the snapshot machinery.
//!
//! Keeps `serde_json::Error` and `std::io::Error` wrapped behind a
//! single concrete type so callers (and future store backends like a
//! Postgres or jsonl store) can return one error from their `save` /
//! `load` calls without leaking dependency details to the caller.

use thiserror::Error;

/// Top-level error type for session storage and snapshot operations.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The requested session id was not present in the store.
    #[error("session not found: {0}")]
    NotFound(String),

    /// Snapshot serialization / deserialization failed.
    #[error("snapshot serde failed: {0}")]
    Serde(#[from] serde_json::Error),

    /// Underlying storage I/O failed (file backends, network, etc).
    #[error("session store io failed: {0}")]
    Io(#[from] std::io::Error),

    /// Snapshot version is newer than this build understands; bumping
    /// [`crate::id::SESSION_VERSION`] is required to read it.
    #[error("snapshot version {found} is newer than supported version {supported}")]
    UnsupportedVersion {
        /// The version embedded in the offending snapshot.
        found: u32,
        /// The highest version this build understands.
        supported: u32,
    },
}
