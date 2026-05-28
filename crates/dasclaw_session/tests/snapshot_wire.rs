//! Wire-format snapshot test for [`SessionSnapshot`].
//!
//! Locks the on-disk JSON shape produced by `serde_json` so any
//! accidental field rename, reorder or addition surfaces as a diff in
//! `cargo insta review`. Bumping the shape on purpose requires both an
//! intentional snapshot review **and** a [`SESSION_VERSION`] bump.
//!
//! Timestamps and session ids are non-deterministic in real life, so
//! we construct a fully literal `SessionSnapshot` value rather than
//! redacting fields from a freshly created one. This keeps the
//! snapshot diff focused on shape, not on test scaffolding.

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{SESSION_VERSION, SessionMetadata, SessionSnapshot};

fn fixture_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-1700000000000-0".to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: None,
        model: Some("test-model".to_string()),
        messages: vec![ChatMessage::user("ping"), ChatMessage::assistant("pong")],
    }
}

#[test]
fn session_snapshot_wire_format_is_stable() {
    let snap = fixture_snapshot();
    insta::assert_json_snapshot!(snap);
}

#[test]
fn session_metadata_wire_format_is_stable() {
    let meta = SessionMetadata::from(&fixture_snapshot());
    insta::assert_json_snapshot!(meta);
}
