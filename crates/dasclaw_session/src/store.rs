//! Storage abstraction over [`SessionSnapshot`].
//!
//! A `SessionStore` is the persistence boundary for sessions. This
//! crate ships one in-memory implementation; a jsonl-on-disk store
//! (claw-code parity, rotation + cleanup) will land in #914 PR-C, and
//! database-backed stores (Postgres, libsql) can be implemented by
//! downstream hosts behind the same trait.
//!
//! The trait is async because realistic backends (filesystem, DB,
//! remote) need to be; the in-memory implementation simply awaits an
//! `RwLock`.

use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::error::SessionError;
use crate::snapshot::{SessionMetadata, SessionSnapshot};

/// Persistence boundary for sessions.
///
/// All methods are `async` so realistic backends (filesystem, DB) can
/// be implemented without blocking the reactor. Backends must be
/// `Send + Sync` so they can be shared across tasks (`Arc<dyn
/// SessionStore>` is the expected usage).
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Insert or replace the snapshot, keyed by `snapshot.session_id`.
    async fn save(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError>;

    /// Return the snapshot for `session_id`, or `Ok(None)` if absent.
    async fn load(&self, session_id: &str) -> Result<Option<SessionSnapshot>, SessionError>;

    /// Cheap index of stored sessions; does not load full message
    /// vectors.
    async fn list(&self) -> Result<Vec<SessionMetadata>, SessionError>;

    /// Remove the snapshot if present. Returns `Ok(())` whether or not
    /// the id existed (idempotent).
    async fn delete(&self, session_id: &str) -> Result<(), SessionError>;
}

/// In-process [`SessionStore`] backed by a `HashMap` under an
/// `RwLock`. Suited for tests, GUI prototypes and headless reuse
/// where session durability across process restarts is not required.
#[derive(Default)]
pub struct InMemorySessionStore {
    inner: RwLock<HashMap<String, SessionSnapshot>>,
}

impl InMemorySessionStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn save(&self, snapshot: &SessionSnapshot) -> Result<(), SessionError> {
        self.inner
            .write()
            .await
            .insert(snapshot.session_id.clone(), snapshot.clone());
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Option<SessionSnapshot>, SessionError> {
        Ok(self.inner.read().await.get(session_id).cloned())
    }

    async fn list(&self) -> Result<Vec<SessionMetadata>, SessionError> {
        Ok(self
            .inner
            .read()
            .await
            .values()
            .map(SessionMetadata::from)
            .collect())
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        self.inner.write().await.remove(session_id);
        Ok(())
    }
}
