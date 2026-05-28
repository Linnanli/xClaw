//! Wire-format snapshot test for the on-disk JSONL session file.
//!
//! Locks the line-by-line framing produced by
//! [`JsonlSessionStore`] so any change to record discriminants
//! (`session_meta` / `message`), field order or field set forces a
//! deliberate `cargo insta review` and a [`SESSION_VERSION`] bump.
//!
//! The fixture is fully literal (no clocks, no temp dirs) so the
//! snapshot diff stays focused on the wire shape.

use std::path::PathBuf;
use std::sync::Arc;

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{JsonlSessionStore, SESSION_VERSION, SessionSnapshot, SessionStore};

fn fixture_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-1700000000000-0".to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: Some(PathBuf::from("/tmp/ws")),
        model: Some("test-model".to_string()),
        messages: vec![ChatMessage::user("ping"), ChatMessage::assistant("pong")],
    }
}

#[tokio::test]
async fn jsonl_wire_format_is_stable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(JsonlSessionStore::new(dir.path().to_path_buf()));
    let snap = fixture_snapshot();
    store.save(&snap).await.expect("save");

    let bytes = tokio::fs::read(store.session_path(&snap.session_id))
        .await
        .expect("read jsonl");
    let text = String::from_utf8(bytes).expect("utf8");

    insta::assert_snapshot!(text);
}
