//! End-to-end tests for [`dasclaw_session::JsonlSessionStore`].
//!
//! Covers the four `SessionStore` contracts plus the rotation /
//! version-rejection requirements from issue #914 PR-C1.

use std::fs;
use std::path::PathBuf;

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{
    JsonlSessionStore, ROTATE_AFTER_BYTES, SESSION_VERSION, SessionError, SessionSnapshot,
    SessionStore,
};
use tempfile::TempDir;

fn fixture_snapshot(session_id: &str, messages: Vec<ChatMessage>) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session_id.to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: Some(PathBuf::from("/tmp/ws")),
        model: Some("test-model".to_string()),
        compaction: None,
        fork: None,
        prompt_history: Vec::new(),
        messages,
    }
}

fn chat(role: &str, content: &str) -> ChatMessage {
    match role {
        "user" => ChatMessage::user(content),
        "assistant" => ChatMessage::assistant(content),
        "system" => ChatMessage::system(content),
        other => panic!("unsupported test role: {other}"),
    }
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_round_trip_preserves_snapshot() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    let snap = fixture_snapshot(
        "session-c1-rt",
        vec![chat("user", "ping"), chat("assistant", "pong")],
    );

    store.save(&snap).await.expect("save");
    let loaded = store
        .load(&snap.session_id)
        .await
        .expect("load")
        .expect("session present");

    assert_eq!(loaded.session_id, snap.session_id);
    assert_eq!(loaded.version, snap.version);
    assert_eq!(loaded.created_at_ms, snap.created_at_ms);
    assert_eq!(loaded.updated_at_ms, snap.updated_at_ms);
    assert_eq!(loaded.model, snap.model);
    assert_eq!(loaded.workspace_root, snap.workspace_root);
    assert_eq!(loaded.messages.len(), snap.messages.len());
    for (l, r) in loaded.messages.iter().zip(snap.messages.iter()) {
        assert_eq!(l.role, r.role);
        assert_eq!(l.content, r.content);
    }
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_list_returns_metadata_without_full_load() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    for (id, model) in [("session-a", "m1"), ("session-b", "m2")] {
        let mut snap = fixture_snapshot(id, vec![chat("user", "hi")]);
        snap.model = Some(model.to_string());
        store.save(&snap).await.expect("save");
    }

    let mut entries = store.list().await.expect("list");
    entries.sort_by(|a, b| a.session_id.cmp(&b.session_id));

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].session_id, "session-a");
    assert_eq!(entries[0].model.as_deref(), Some("m1"));
    assert_eq!(entries[0].version, SESSION_VERSION);
    assert_eq!(entries[1].session_id, "session-b");
    assert_eq!(entries[1].model.as_deref(), Some("m2"));
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_missing_session_load_returns_none() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    let res = store.load("session-does-not-exist").await.expect("load");
    assert!(res.is_none());
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_delete_is_idempotent() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    let snap = fixture_snapshot("session-del", vec![chat("user", "hi")]);
    store.save(&snap).await.expect("save");

    store.delete(&snap.session_id).await.expect("first delete");
    store
        .delete(&snap.session_id)
        .await
        .expect("second delete is idempotent");

    assert!(store.load(&snap.session_id).await.expect("load").is_none());
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_future_version_is_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    let mut snap = fixture_snapshot("session-fv", vec![chat("user", "hi")]);
    snap.version = SESSION_VERSION + 99;

    // The store is allowed to either reject on save or on load; the
    // contract is that callers never see a snapshot they cannot
    // interpret. We exercise the load path because that's what hits
    // mismatched-version files written by a future binary.
    let path = dir.path().join(format!("{}.jsonl", snap.session_id));
    let line = format!(
        "{{\"type\":\"session_meta\",\"version\":{},\"session_id\":\"{}\",\"created_at_ms\":1,\"updated_at_ms\":1}}\n",
        snap.version, snap.session_id
    );
    fs::write(&path, line).expect("write fixture");

    let err = store
        .load(&snap.session_id)
        .await
        .expect_err("future version must be rejected");
    match err {
        SessionError::UnsupportedVersion { found, supported } => {
            assert_eq!(found, SESSION_VERSION + 99);
            assert_eq!(supported, SESSION_VERSION);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_rotates_after_threshold() {
    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    // Build a snapshot large enough that a single save exceeds the
    // rotation threshold. We pad message content with a known string.
    let pad = "x".repeat(2048);
    let mut messages = Vec::new();
    while (messages.len() as u64) * (pad.len() as u64 + 64) < ROTATE_AFTER_BYTES + 8 * 1024 {
        messages.push(chat("user", &pad));
    }
    let snap = fixture_snapshot("session-rot", messages);

    // First save creates the file.
    store.save(&snap).await.expect("first save");
    // Second save must observe the existing oversized file and rotate
    // it before writing.
    store
        .save(&snap)
        .await
        .expect("second save triggers rotate");

    let entries: Vec<_> = fs::read_dir(dir.path())
        .expect("readdir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let rotated: Vec<&String> = entries
        .iter()
        .filter(|n| n.starts_with("session-rot.rot-") && n.ends_with(".jsonl"))
        .collect();
    assert!(
        !rotated.is_empty(),
        "expected at least one rotated file in {entries:?}"
    );
    assert!(
        entries.iter().any(|n| n == "session-rot.jsonl"),
        "live session file should still exist in {entries:?}"
    );
}

#[tokio::test]
async fn req_dasclaw_session_c1_jsonl_caps_rotated_history_at_three() {
    use dasclaw_session::MAX_ROTATED_FILES;

    let dir = TempDir::new().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());

    let pad = "y".repeat(4096);
    let mut messages = Vec::new();
    while (messages.len() as u64) * (pad.len() as u64 + 64) < ROTATE_AFTER_BYTES + 8 * 1024 {
        messages.push(chat("user", &pad));
    }
    let snap = fixture_snapshot("session-cap", messages);

    // Repeated saves should trigger multiple rotations but the on-disk
    // rotated file count must never exceed `MAX_ROTATED_FILES`.
    for _ in 0..(MAX_ROTATED_FILES + 3) {
        store.save(&snap).await.expect("save");
    }

    let rotated_count = fs::read_dir(dir.path())
        .expect("readdir")
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("session-cap.rot-") && name.ends_with(".jsonl")
        })
        .count();
    assert!(
        rotated_count <= MAX_ROTATED_FILES,
        "rotated history should be capped at {MAX_ROTATED_FILES}, got {rotated_count}"
    );
}
