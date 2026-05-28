//! Tests for [`SessionFork`] + [`SessionSnapshot::fork`] +
//! [`SessionPromptEntry`] + [`SessionSnapshot::record_prompt`].
//!
//! Behavior locked here:
//! - `fork(branch_name)` creates a fresh session_id (different from
//!   parent), copies messages / compaction / workspace_root / model /
//!   prompt_history verbatim, records `SessionFork { parent_session_id,
//!   branch_name }`, and resets `created_at_ms` / `updated_at_ms` to
//!   the current wall-clock so the child has its own provenance.
//! - `record_prompt(text)` appends a `SessionPromptEntry` carrying
//!   the current wall-clock and the text, and bumps `updated_at_ms`.
//! - `compact_oldest` (from PR-C2) must NOT clear `prompt_history` —
//!   the prompt log is the GUI's audit trail and survives every
//!   compaction pass.

use std::path::PathBuf;

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{
    SESSION_VERSION, SessionCompaction, SessionFork, SessionPromptEntry, SessionSnapshot,
};

fn fresh_snapshot() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-parent-1".to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: Some(PathBuf::from("/tmp/ws")),
        model: Some("test-model".to_string()),
        compaction: None,
        fork: None,
        prompt_history: Vec::new(),
        messages: vec![
            ChatMessage::user("turn 1"),
            ChatMessage::assistant("reply 1"),
        ],
    }
}

#[test]
fn req_dasclaw_session_c3_fork_assigns_new_session_id() {
    let parent = fresh_snapshot();
    let child = parent.fork(Some("experiment".to_string()));
    assert_ne!(child.session_id, parent.session_id);
    assert!(
        !child.session_id.is_empty(),
        "child session_id must be non-empty"
    );
}

#[test]
fn req_dasclaw_session_c3_fork_records_parent_id_and_branch_name() {
    let parent = fresh_snapshot();
    let child = parent.fork(Some("experiment".to_string()));
    let fork = child.fork.as_ref().expect("child must record fork");
    assert_eq!(
        fork,
        &SessionFork {
            parent_session_id: "session-parent-1".to_string(),
            branch_name: Some("experiment".to_string()),
        }
    );
}

#[test]
fn req_dasclaw_session_c3_fork_without_branch_name_keeps_none() {
    let parent = fresh_snapshot();
    let child = parent.fork(None);
    assert_eq!(
        child.fork.as_ref().expect("fork recorded").branch_name,
        None
    );
}

#[test]
fn req_dasclaw_session_c3_fork_clones_messages_compaction_and_prompts() {
    let mut parent = fresh_snapshot();
    parent.compaction = Some(SessionCompaction {
        count: 2,
        removed_message_count: 4,
        summary: "earlier summary".to_string(),
    });
    parent.prompt_history.push(SessionPromptEntry {
        timestamp_ms: 1_699_999_900_000,
        text: "first prompt".to_string(),
    });

    let child = parent.fork(None);
    assert_eq!(child.messages.len(), parent.messages.len());
    for (c, p) in child.messages.iter().zip(parent.messages.iter()) {
        assert_eq!(c.role, p.role);
        assert_eq!(c.content, p.content);
    }
    assert_eq!(child.compaction, parent.compaction);
    assert_eq!(child.prompt_history, parent.prompt_history);
    assert_eq!(child.workspace_root, parent.workspace_root);
    assert_eq!(child.model, parent.model);
}

#[test]
fn req_dasclaw_session_c3_fork_resets_timestamps_to_now() {
    let parent = fresh_snapshot();
    let before = current_ms();
    let child = parent.fork(None);
    let after = current_ms();
    assert!(
        child.created_at_ms >= before && child.created_at_ms <= after,
        "child.created_at_ms ({}) must be wall-clock now in [{}, {}]",
        child.created_at_ms,
        before,
        after
    );
    assert_eq!(child.updated_at_ms, child.created_at_ms);
}

#[test]
fn req_dasclaw_session_c3_record_prompt_appends_entry_and_touches_updated_at() {
    let mut snap = fresh_snapshot();
    snap.updated_at_ms = 0; // pin so we can detect any forward motion
    let before = current_ms();
    snap.record_prompt("what's next?");
    let after = current_ms();

    assert_eq!(snap.prompt_history.len(), 1);
    let entry = &snap.prompt_history[0];
    assert_eq!(entry.text, "what's next?");
    assert!(
        entry.timestamp_ms >= before && entry.timestamp_ms <= after,
        "prompt timestamp ({}) must be wall-clock now in [{}, {}]",
        entry.timestamp_ms,
        before,
        after
    );
    assert!(
        snap.updated_at_ms >= before,
        "record_prompt must touch updated_at_ms"
    );
}

#[test]
fn req_dasclaw_session_c3_compact_oldest_preserves_prompt_history() {
    let mut snap = fresh_snapshot();
    snap.prompt_history.push(SessionPromptEntry {
        timestamp_ms: 1_699_999_900_000,
        text: "p1".to_string(),
    });
    snap.prompt_history.push(SessionPromptEntry {
        timestamp_ms: 1_699_999_910_000,
        text: "p2".to_string(),
    });
    let before = snap.prompt_history.clone();

    snap.compact_oldest(2, |_| "summary".to_string());

    assert_eq!(
        snap.prompt_history, before,
        "compact_oldest must NOT touch prompt_history"
    );
}

#[test]
fn req_dasclaw_session_c3_session_fork_and_prompt_entry_serde_round_trip() {
    let fork = SessionFork {
        parent_session_id: "session-parent-1".to_string(),
        branch_name: Some("feature/x".to_string()),
    };
    let s = serde_json::to_string(&fork).expect("fork ser");
    let back: SessionFork = serde_json::from_str(&s).expect("fork de");
    assert_eq!(back, fork);

    let entry = SessionPromptEntry {
        timestamp_ms: 1_699_999_900_000,
        text: "hello".to_string(),
    };
    let s = serde_json::to_string(&entry).expect("entry ser");
    let back: SessionPromptEntry = serde_json::from_str(&s).expect("entry de");
    assert_eq!(back, entry);
}

fn current_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_millis(),
    )
    .expect("ms fits in u64 for any sane wall-clock")
}
