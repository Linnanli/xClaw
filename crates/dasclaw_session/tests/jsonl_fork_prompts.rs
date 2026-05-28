//! Wire-format tests for PR-C3: fork + prompt_history must serialize
//! and round-trip through the JsonlSessionStore the same way
//! claw-code's `session.jsonl` lays them out:
//!
//! - `fork` is embedded inside the `session_meta` record (not a
//!   separate line). Absent when `None`.
//! - `prompt_history` entries are independent `{"type":"prompt_history",
//!   "timestamp_ms":..., "text":...}` lines sitting between the
//!   `compaction` record (if any) and the first `message` record.

use std::path::PathBuf;

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{
    JsonlSessionStore, SESSION_VERSION, SessionCompaction, SessionFork, SessionPromptEntry,
    SessionSnapshot, SessionStore,
};

fn fixture() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-child-1".to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_001_000,
        updated_at_ms: 1_700_000_002_000,
        workspace_root: Some(PathBuf::from("/tmp/ws")),
        model: Some("test-model".to_string()),
        compaction: Some(SessionCompaction {
            count: 1,
            removed_message_count: 3,
            summary: "earlier discussion summarized".to_string(),
        }),
        fork: Some(SessionFork {
            parent_session_id: "session-parent-1".to_string(),
            branch_name: Some("experiment".to_string()),
        }),
        prompt_history: vec![
            SessionPromptEntry {
                timestamp_ms: 1_699_999_900_000,
                text: "first prompt".to_string(),
            },
            SessionPromptEntry {
                timestamp_ms: 1_699_999_910_000,
                text: "second prompt".to_string(),
            },
        ],
        messages: vec![
            ChatMessage::system("earlier discussion summarized"),
            ChatMessage::user("now what?"),
        ],
    }
}

#[tokio::test]
async fn req_dasclaw_session_c3_jsonl_round_trip_preserves_fork_and_prompts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());
    let snap = fixture();
    store.save(&snap).await.expect("save");
    let loaded = store
        .load(&snap.session_id)
        .await
        .expect("load")
        .expect("session present");

    assert_eq!(loaded.fork, snap.fork);
    assert_eq!(loaded.prompt_history, snap.prompt_history);
    assert_eq!(loaded.compaction, snap.compaction);
    assert_eq!(loaded.messages.len(), snap.messages.len());
    for (l, r) in loaded.messages.iter().zip(snap.messages.iter()) {
        assert_eq!(l.role, r.role);
        assert_eq!(l.content, r.content);
    }
}

#[tokio::test]
async fn req_dasclaw_session_c3_jsonl_wire_layout_matches_claw_code() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());
    let snap = fixture();
    store.save(&snap).await.expect("save");

    let bytes = tokio::fs::read(store.session_path(&snap.session_id))
        .await
        .expect("read");
    let text = String::from_utf8(bytes).expect("utf8");
    let lines: Vec<&str> = text.lines().collect();

    // Line 1: session_meta with fork embedded
    assert!(
        lines[0].contains(r#""type":"session_meta""#),
        "line 1 must be session_meta: {:?}",
        lines.first()
    );
    assert!(
        lines[0].contains(r#""fork""#)
            && lines[0].contains(r#""parent_session_id":"session-parent-1""#)
            && lines[0].contains(r#""branch_name":"experiment""#),
        "fork must be embedded inside session_meta, got line: {:?}",
        lines.first()
    );

    // Line 2: compaction
    assert!(
        lines[1].contains(r#""type":"compaction""#),
        "line 2 must be compaction: {:?}",
        lines.get(1)
    );

    // Lines 3 & 4: prompt_history
    assert!(
        lines[2].contains(r#""type":"prompt_history""#)
            && lines[2].contains(r#""text":"first prompt""#),
        "line 3 must be 1st prompt_history record: {:?}",
        lines.get(2)
    );
    assert!(
        lines[3].contains(r#""type":"prompt_history""#)
            && lines[3].contains(r#""text":"second prompt""#),
        "line 4 must be 2nd prompt_history record: {:?}",
        lines.get(3)
    );

    // Remaining lines: messages
    for (i, line) in lines.iter().enumerate().skip(4) {
        assert!(
            line.contains(r#""type":"message""#),
            "line {} must be a message record, got: {:?}",
            i + 1,
            line
        );
    }
}

#[tokio::test]
async fn req_dasclaw_session_c3_jsonl_skips_absent_fork_and_empty_prompts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());
    let mut snap = fixture();
    snap.fork = None;
    snap.prompt_history.clear();
    snap.compaction = None;
    store.save(&snap).await.expect("save");

    let bytes = tokio::fs::read(store.session_path(&snap.session_id))
        .await
        .expect("read");
    let text = String::from_utf8(bytes).expect("utf8");

    assert!(
        !text.contains(r#""type":"prompt_history""#),
        "empty prompt_history must not emit any records"
    );
    assert!(
        !text.contains(r#""fork""#),
        "None fork must not appear in session_meta: {text}"
    );
}
