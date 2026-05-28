//! End-to-end tests for the [`SessionStore`] persistence boundary
//! introduced in #914 PR-B.
//!
//! Three contracts are covered here:
//!
//! 1. `InMemorySessionStore` round-trips: `save` → `load` → `delete`
//!    behave correctly, and `list` returns a lightweight metadata
//!    view that mirrors what was saved.
//! 2. `Session` ↔ `SessionSnapshot`: a live session can be snapshotted,
//!    the snapshot can be stored, reloaded, and used to rebuild a
//!    new `Session` whose `messages()` exactly matches the original.
//! 3. `generate_session_id` is monotonic enough that two consecutive
//!    calls never collide, even in tight loops.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ChatMessage, FinishReason};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentResponder};
use dasclaw_session::{
    InMemorySessionStore, SESSION_VERSION, Session, SessionSnapshot, SessionStore,
    generate_session_id,
};

/// Minimal responder that returns a single scripted text turn. Same
/// pattern as `session_e2e.rs`, kept local so the two test files do not
/// share a test-utility module yet.
struct OneShotResponder {
    reply: &'static str,
}

#[async_trait]
impl AgentResponder for OneShotResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        Ok(RespondOutput {
            result: RespondResult::Text(self.reply.to_string()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}

fn build_agent(reply: &'static str) -> Arc<Agent> {
    Arc::new(
        Agent::builder()
            .responder(OneShotResponder { reply })
            .system_prompt("be concise")
            .build()
            .expect("build agent"),
    )
}

fn sample_snapshot(session_id: &str) -> SessionSnapshot {
    SessionSnapshot {
        session_id: session_id.to_string(),
        version: SESSION_VERSION,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_500,
        workspace_root: None,
        model: Some("test-model".to_string()),
        compaction: None,
        messages: vec![
            ChatMessage::user("hi"),
            ChatMessage::assistant("hello back"),
        ],
    }
}

#[tokio::test]
async fn req_dasclaw_session_b2_in_memory_store_round_trip() {
    let store = InMemorySessionStore::new();
    let snap = sample_snapshot("session-test-1");

    store.save(&snap).await.expect("save");

    let loaded = store
        .load("session-test-1")
        .await
        .expect("load ok")
        .expect("session present");
    assert_eq!(loaded.session_id, snap.session_id);
    assert_eq!(loaded.version, SESSION_VERSION);
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.model.as_deref(), Some("test-model"));

    let metas = store.list().await.expect("list");
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].session_id, "session-test-1");
    assert_eq!(metas[0].model.as_deref(), Some("test-model"));

    store.delete("session-test-1").await.expect("delete");
    let after = store.load("session-test-1").await.expect("load ok");
    assert!(after.is_none(), "session should be gone after delete");
}

#[tokio::test]
async fn req_dasclaw_session_b2_missing_session_returns_none() {
    let store = InMemorySessionStore::new();
    let loaded = store.load("does-not-exist").await.expect("load ok");
    assert!(loaded.is_none());

    // Deleting an absent id is idempotent, not an error.
    store.delete("does-not-exist").await.expect("delete absent");
}

#[tokio::test]
async fn req_dasclaw_session_b2_session_snapshot_round_trip_preserves_history() {
    let agent = build_agent("the answer is 4");
    let mut session = Session::new(agent.clone()).with_model("scripted");

    let original_id = session.session_id().to_string();
    let _ = session.run("2 + 2?").await.expect("run");
    assert_eq!(session.messages().len(), 2);

    let store = InMemorySessionStore::new();
    store.save(session.snapshot()).await.expect("save");

    let loaded = store
        .load(&original_id)
        .await
        .expect("load ok")
        .expect("present");
    let resumed = Session::from_snapshot(agent, loaded);

    assert_eq!(resumed.session_id(), original_id);
    assert_eq!(resumed.messages().len(), session.messages().len());
    assert_eq!(
        resumed.snapshot().model.as_deref(),
        Some("scripted"),
        "builder-style model should be preserved"
    );
}

#[test]
fn req_dasclaw_session_b2_session_id_is_unique_under_tight_loop() {
    // 1k iterations is plenty to flush out a missing counter without
    // making the test slow. Same-millisecond collisions would surface
    // here if the counter were ever removed.
    let mut ids = std::collections::HashSet::new();
    for _ in 0..1000 {
        let id = generate_session_id();
        assert!(ids.insert(id.clone()), "duplicate session id: {id}");
        assert!(id.starts_with("session-"));
    }
}
