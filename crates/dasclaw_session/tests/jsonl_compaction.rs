//! End-to-end test: SessionCompaction survives a JsonlSessionStore
//! round-trip and shows up between session_meta and the message
//! records (claw-code wire-parity).

use std::path::PathBuf;

use dasclaw_core::messages::ChatMessage;
use dasclaw_session::{
    JsonlSessionStore, SESSION_VERSION, SessionCompaction, SessionSnapshot, SessionStore,
};

fn fixture() -> SessionSnapshot {
    SessionSnapshot {
        session_id: "session-1700000000000-0".to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: Some(PathBuf::from("/tmp/ws")),
        model: Some("test-model".to_string()),
        compaction: Some(SessionCompaction {
            count: 1,
            removed_message_count: 3,
            summary: "earlier discussion summarized".to_string(),
        }),
        messages: vec![
            ChatMessage::system("earlier discussion summarized"),
            ChatMessage::user("now what?"),
        ],
    }
}

#[tokio::test]
async fn req_dasclaw_session_c2_jsonl_round_trip_preserves_compaction() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());
    let snap = fixture();

    store.save(&snap).await.expect("save");
    let loaded = store
        .load(&snap.session_id)
        .await
        .expect("load")
        .expect("session present");

    let comp = loaded
        .compaction
        .as_ref()
        .expect("compaction must survive jsonl round trip");
    assert_eq!(comp.count, 1);
    assert_eq!(comp.removed_message_count, 3);
    assert_eq!(comp.summary, "earlier discussion summarized");
    assert_eq!(loaded.messages.len(), 2);
}

#[tokio::test]
async fn req_dasclaw_session_c2_jsonl_wire_records_compaction_between_meta_and_messages() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = JsonlSessionStore::new(dir.path().to_path_buf());
    let snap = fixture();
    store.save(&snap).await.expect("save");

    let bytes = tokio::fs::read(store.session_path(&snap.session_id))
        .await
        .expect("read");
    let text = String::from_utf8(bytes).expect("utf8");
    let lines: Vec<&str> = text.lines().collect();

    assert!(
        lines[0].contains(r#""type":"session_meta""#),
        "line 1 must be session_meta, got {:?}",
        lines.first()
    );
    assert!(
        lines[1].contains(r#""type":"compaction""#),
        "line 2 must be compaction, got {:?}",
        lines.get(1)
    );
    assert!(
        lines[1].contains(r#""count":1"#) && lines[1].contains(r#""removed_message_count":3"#),
        "compaction record must carry its fields: {:?}",
        lines.get(1)
    );
    for (i, line) in lines.iter().enumerate().skip(2) {
        assert!(
            line.contains(r#""type":"message""#),
            "line {} must be a message record after compaction, got {:?}",
            i + 1,
            line
        );
    }
}
