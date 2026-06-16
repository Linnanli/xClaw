//! End-to-end tests for issue #907: cancel handle + multi-turn `Session`.
//!
//! Two behavioural contracts are covered here:
//!
//! 1. `Agent::cancel_handle()` — when the token is already cancelled,
//!    `Agent::invoke` exits at the first signal check with
//!    [`AgentError::Stopped`] without ever calling the responder.
//! 2. `Session::invoke` — running twice in a row carries the conversation
//!    history into the second call, so the responder sees the prior user
//!    and assistant turns in `ReasoningContext::messages`. External
//!    cancel on a session also surfaces as [`AgentError::Stopped`].
//!
//! ### Why we test pre-cancellation rather than mid-flight cancellation
//!
//! The agentic loop checks `check_signals` once per iteration boundary.
//! A text response is terminal (the loop returns immediately on it), so
//! mid-flight cancellation only matters when a tool call splits the
//! conversation into ≥2 iterations. Wiring a full `ToolExecutor` mock
//! solely to prove the token is read on every iteration adds noise here;
//! the iteration-boundary contract is already covered by
//! `dasclaw_core::agentic_loop` unit tests (search for
//! `with_signal(LoopSignal::Stop)`), and this file focuses on the new
//! `Agent::cancel_handle` plumbing instead.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ChatMessage, FinishReason};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentError, AgentResponder, AgentRunOptions};
use dasclaw_session::Session;
use tokio_util::sync::CancellationToken;

/// Scripted responder: returns text outputs in order, and records the
/// length of `ctx.messages` it saw on each call so tests can assert how
/// much history was carried in.
struct ScriptedResponder {
    script: tokio::sync::Mutex<Vec<&'static str>>,
    seen_message_counts: tokio::sync::Mutex<Vec<usize>>,
}

impl ScriptedResponder {
    fn new(script: Vec<&'static str>) -> Self {
        Self {
            script: tokio::sync::Mutex::new(script),
            seen_message_counts: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    async fn seen_counts(&self) -> Vec<usize> {
        self.seen_message_counts.lock().await.clone()
    }
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        self.seen_message_counts
            .lock()
            .await
            .push(ctx.messages.len());
        let mut s = self.script.lock().await;
        if s.is_empty() {
            return Err("script exhausted".into());
        }
        let text = s.remove(0);
        Ok(RespondOutput {
            result: RespondResult::Text(text.to_string()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}

#[tokio::test]
async fn req_dasclaw_runtime_session_b1_run_twice_carries_history() {
    let responder = Arc::new(ScriptedResponder::new(vec!["first reply", "second reply"]));
    let agent = Arc::new(
        Agent::builder()
            .responder_arc(responder.clone())
            .system_prompt("be concise")
            .build()
            .expect("build agent"),
    );

    let mut session = Session::new(agent);

    let first = session
        .invoke("hello", AgentRunOptions::invoke())
        .await
        .expect("first run");
    assert_eq!(first.text, "first reply");

    let second = session
        .invoke("again", AgentRunOptions::invoke())
        .await
        .expect("second run");
    assert_eq!(second.text, "second reply");

    // Responder saw growing context across calls:
    //   call 1: [user("hello")]
    //   call 2: [user("hello"), assistant("first reply"), user("again")]
    let counts = responder.seen_counts().await;
    assert_eq!(
        counts,
        vec![1, 3],
        "responder must see the prior turn carried into the 2nd call"
    );

    // Session exposes the accumulated history for the GUI to render.
    let history: Vec<&ChatMessage> = session.messages().iter().collect();
    assert_eq!(history.len(), 4, "expected user/assistant x2: {history:?}");
}

#[tokio::test]
async fn req_dasclaw_runtime_agent_b1_cancel_handle_stops_loop_before_responder() {
    let token = CancellationToken::new();
    // Cancel before the loop even starts; the first iteration's signal
    // check must short-circuit to `Stopped` and the scripted responder
    // must never be polled.
    token.cancel();

    let responder = ScriptedResponder::new(vec!["never reached"]);
    let agent = Agent::builder()
        .responder(responder)
        .cancellation_token(token.clone())
        .build()
        .expect("build agent");

    let handle = agent
        .cancel_handle()
        .expect("cancel_handle returns the configured token");
    assert!(handle.is_cancelled(), "we just cancelled it");

    let err = agent
        .invoke("please stop", AgentRunOptions::invoke())
        .await
        .expect_err("loop should stop, not produce a response");
    assert!(
        matches!(err, AgentError::Stopped),
        "expected AgentError::Stopped, got {err:?}"
    );
}

#[tokio::test]
async fn req_dasclaw_runtime_agent_b1_cancel_handle_returns_none_when_unset() {
    let responder = ScriptedResponder::new(vec!["hello world"]);
    let agent = Agent::builder()
        .responder(responder)
        .build()
        .expect("build");
    assert!(
        agent.cancel_handle().is_none(),
        "cancel_handle must be None when no token was wired"
    );
}

#[tokio::test]
async fn req_dasclaw_runtime_session_b1_external_cancel_stops_session_run() {
    let token = CancellationToken::new();
    token.cancel();

    let responder = ScriptedResponder::new(vec!["never reached"]);
    let agent = Arc::new(
        Agent::builder()
            .responder(responder)
            .cancellation_token(token)
            .build()
            .expect("build agent"),
    );

    let mut session = Session::new(agent);
    let err = session
        .invoke("anything", AgentRunOptions::invoke())
        .await
        .expect_err("session.invoke must propagate Stopped");
    assert!(
        matches!(err, AgentError::Stopped),
        "expected AgentError::Stopped, got {err:?}"
    );
}

/// Regression for PR-C2 reviewer finding: `Session::compact_oldest`
/// must advance `updated_at_ms` whenever a compaction pass actually
/// runs — including the edge case where `remove_count == 1` and the
/// post-compaction message count happens to equal the pre-compaction
/// one (drain 1 + insert 1 summary = same length, but the snapshot
/// has nevertheless changed).
#[tokio::test]
async fn req_dasclaw_session_c2_session_compact_oldest_touches_updated_at_ms() {
    use dasclaw_session::SessionSnapshot;

    let responder = ScriptedResponder::new(vec!["unused"]);
    let agent = Arc::new(
        Agent::builder()
            .responder(responder)
            .build()
            .expect("build agent"),
    );

    let mut snap = SessionSnapshot::fresh();
    snap.updated_at_ms = 0; // pin to a fixed value so we can detect any forward motion
    snap.messages
        .push(ChatMessage::user("only original message"));

    let mut session = Session::from_snapshot(agent, snap);
    let before = session.snapshot().updated_at_ms;

    session.compact_oldest(1, |_removed| "summary".to_string());

    let after = session.snapshot().updated_at_ms;
    assert!(
        after > before,
        "updated_at_ms must advance on a real compaction pass (before={before}, after={after})"
    );
    assert!(
        session.snapshot().compaction.is_some(),
        "compaction field must be recorded"
    );
}
