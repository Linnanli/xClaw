//! Tests for [`SessionCompaction`] + [`SessionSnapshot::compaction`]
//! plus the [`Session::compact_oldest`] API.
//!
//! Behavior locked here:
//! - `compact_oldest(n, summarizer)` removes the first `n` messages
//!   (clamped to `messages.len()`), invokes `summarizer` on those
//!   removed messages, and prepends a `ChatMessage::system` carrying
//!   the returned summary.
//! - `SessionSnapshot.compaction` accumulates: a second
//!   `compact_oldest` call bumps `count` to 2.
//! - `compact_oldest(0, _)` is a no-op (no summarizer call, no
//!   `compaction` field mutation).
//! - The first system message is the summary placeholder, not a
//!   counted "message" for protocol purposes.

use std::path::PathBuf;

use dasclaw_core::messages::{ChatMessage, Role};
use dasclaw_session::{SESSION_VERSION, SessionCompaction, SessionSnapshot};

fn snapshot_with_messages(messages: Vec<ChatMessage>) -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-test-compact".to_string(),
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

#[test]
fn req_dasclaw_session_c2_compact_oldest_removes_n_and_injects_summary() {
    let mut snap = snapshot_with_messages(vec![
        ChatMessage::user("m1"),
        ChatMessage::assistant("m2"),
        ChatMessage::user("m3"),
        ChatMessage::assistant("m4"),
        ChatMessage::user("m5"),
    ]);

    let mut captured = None;
    snap.compact_oldest(3, |removed| {
        captured = Some(removed.len());
        "summary-of-first-three".to_string()
    });

    assert_eq!(captured, Some(3), "summarizer must see exactly 3 removed");
    assert_eq!(snap.messages.len(), 1 + 2, "1 summary + 2 retained");
    assert_eq!(snap.messages[0].role, Role::System);
    assert!(snap.messages[0].content.contains("summary-of-first-three"));
    assert_eq!(snap.messages[1].content, "m4");
    assert_eq!(snap.messages[2].content, "m5");

    let comp = snap.compaction.as_ref().expect("compaction recorded");
    assert_eq!(comp.count, 1);
    assert_eq!(comp.removed_message_count, 3);
    assert_eq!(comp.summary, "summary-of-first-three");
}

#[test]
fn req_dasclaw_session_c2_compact_oldest_accumulates_count_on_repeat() {
    let mut snap = snapshot_with_messages(vec![
        ChatMessage::user("a"),
        ChatMessage::assistant("b"),
        ChatMessage::user("c"),
        ChatMessage::assistant("d"),
    ]);
    snap.compact_oldest(2, |_| "s1".to_string());
    snap.compact_oldest(1, |_| "s2".to_string());

    let comp = snap.compaction.expect("compaction recorded");
    assert_eq!(comp.count, 2);
    assert_eq!(comp.removed_message_count, 1, "tracks the latest pass");
    assert_eq!(comp.summary, "s2");
}

#[test]
fn req_dasclaw_session_c2_compact_oldest_zero_is_noop() {
    let original = vec![ChatMessage::user("only")];
    let mut snap = snapshot_with_messages(original.clone());

    let mut called = false;
    snap.compact_oldest(0, |_| {
        called = true;
        "should-not-run".to_string()
    });

    assert!(!called, "summarizer must not run when remove_count is 0");
    assert!(snap.compaction.is_none());
    assert_eq!(snap.messages.len(), original.len());
}

#[test]
fn req_dasclaw_session_c2_compact_oldest_clamps_to_available() {
    let mut snap = snapshot_with_messages(vec![ChatMessage::user("only-one")]);
    snap.compact_oldest(99, |removed| {
        assert_eq!(removed.len(), 1, "must clamp to available count");
        "everything".to_string()
    });

    assert_eq!(snap.messages.len(), 1, "1 summary, 0 retained");
    assert_eq!(snap.messages[0].role, Role::System);
    let comp = snap.compaction.as_ref().expect("compaction recorded");
    assert_eq!(
        comp.removed_message_count, 1,
        "records the actual removed count"
    );
}

#[test]
fn req_dasclaw_session_c2_session_compaction_serde_round_trip() {
    let comp = SessionCompaction {
        count: 7,
        removed_message_count: 42,
        summary: "previously chatted about widgets".to_string(),
    };
    let json = serde_json::to_string(&comp).expect("serialize");
    let back: SessionCompaction = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, comp);
}
