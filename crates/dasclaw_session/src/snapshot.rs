//! Serializable, agent-independent snapshot of a [`Session`](crate::Session).
//!
//! `Session` itself holds an `Arc<Agent>` (live trait objects, futures,
//! mutex state) that cannot be serialized. Persistence and `SessionStore`
//! backends operate on the [`SessionSnapshot`] view, which carries only
//! the conversation history plus identifying metadata.
//!
//! Round-trip is:
//!
//! ```text
//! Session ──snapshot()──▶ SessionSnapshot ──serde──▶ bytes
//!   ▲                                                  │
//!   └──── Session::from_snapshot(agent, snap) ◀────────┘
//! ```
//!
//! The wire format is locked by an insta snapshot test
//! (`tests/snapshot_wire.rs`) so any accidental field rename or shape
//! change shows up as a review-time diff and forces a deliberate
//! [`crate::id::SESSION_VERSION`] bump.

use std::path::PathBuf;

use dasclaw_core::messages::ChatMessage;
use serde::{Deserialize, Serialize};

use crate::id::{SESSION_VERSION, current_time_millis, generate_session_id};

/// Lightweight identifier + timestamps view returned from
/// [`crate::store::SessionStore::list`].
///
/// Carrying the full `messages` vector through `list()` would force
/// every store backend to deserialize entire conversations just to
/// build the index; this view stays cheap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub session_id: String,
    /// Wire-format version of the underlying snapshot.
    pub version: u32,
    /// Unix milliseconds when the session was created.
    pub created_at_ms: u64,
    /// Unix milliseconds of the last write.
    pub updated_at_ms: u64,
    /// Optional model name that drove this session (for resume UX).
    pub model: Option<String>,
}

impl From<&SessionSnapshot> for SessionMetadata {
    fn from(snap: &SessionSnapshot) -> Self {
        Self {
            session_id: snap.session_id.clone(),
            version: snap.version,
            created_at_ms: snap.created_at_ms,
            updated_at_ms: snap.updated_at_ms,
            model: snap.model.clone(),
        }
    }
}

/// All state that `Session` needs to persist across process restarts.
///
/// Field layout follows the `claw-code::session::Session` core fields
/// (session_id / version / timestamps / workspace_root / model /
/// messages) so a future jsonl store (#914 PR-C) can match its
/// `session_meta` record shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Unique session identifier; see [`crate::id::generate_session_id`].
    pub session_id: String,
    /// Wire-format version. Loaders reject snapshots newer than the
    /// supported [`crate::id::SESSION_VERSION`].
    pub version: u32,
    /// Unix milliseconds at session creation.
    pub created_at_ms: u64,
    /// Unix milliseconds of the last mutation. Updated by
    /// [`crate::Session::run`] on successful completion.
    pub updated_at_ms: u64,
    /// Optional bound workspace; recorded for the UI / future
    /// concurrency guards. Not interpreted by the snapshot logic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
    /// Optional model name the session was driven with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Full conversation history, in turn order.
    pub messages: Vec<ChatMessage>,
}

impl SessionSnapshot {
    /// Build a fresh snapshot with a generated id and current time.
    /// Used by [`crate::Session::new`].
    #[must_use]
    pub fn fresh() -> Self {
        let now = current_time_millis();
        Self {
            session_id: generate_session_id(),
            version: SESSION_VERSION,
            created_at_ms: now,
            updated_at_ms: now,
            workspace_root: None,
            model: None,
            messages: Vec::new(),
        }
    }

    /// Refresh `updated_at_ms` to the current wall-clock time.
    pub fn touch(&mut self) {
        self.updated_at_ms = current_time_millis();
    }
}
