//! ADR-153 §1.1 e9 (A3 — L6 outbound credential leak in tool result).
//!
//! Setup: scripted responder emits one tool call; the executor returns a
//! `ToolResult` whose content embeds an OpenAI-style secret
//! (`sk-leaked-key-AAAA1111BBBB2222`). The agent is wired with a
//! [`SanitizingDisplayGate`] that rewrites every
//! `EgressKind::UserDisplay` payload containing the `sk-` prefix to a
//! fixed `[REDACTED]` placeholder, wrapped in a [`RecordingEgressGate`]
//! so the test can assert which kind tripped the redaction.
//!
//! Contract verified (matches ADR-153 §1.1 e9 row):
//!   * The second LLM turn's `ReasoningContext` contains the tool_result
//!     with `[REDACTED]` substituted; the raw secret never reaches the
//!     next LLM turn.
//!   * At least one `EgressKind::UserDisplay` check resulted in
//!     `EgressDecision::Redact { .. }`.
//!   * The final reply does not echo the raw secret.

use std::sync::Arc;

use dasclaw_core::hooks::{EgressDecision, EgressKind, HookBundle};
use dasclaw_core::messages::Role;
use dasclaw_runtime::{Agent, AgentRunOptions};

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{
    LeakyToolExecutor, RecordingEgressGate, SanitizingDisplayGate, ScriptedResponder, text_turn,
    tool_call_turn,
};

const RAW_SECRET: &str = "sk-leaked-key-AAAA1111BBBB2222";
const LEAKY_CONTENT: &str =
    "Found env var: OPENAI_API_KEY=sk-leaked-key-AAAA1111BBBB2222 export it now";
const SANITIZED_CONTENT: &str = "Found env var: OPENAI_API_KEY=[REDACTED] export it now";
const REDACTED_MARKER: &str = "[REDACTED]";
const FINAL_REPLY: &str = "done: credentials masked";

#[tokio::test]
async fn req_dasclaw_cli_safety_e9_tool_result_credential_redacted() {
    let responder = Arc::new(ScriptedResponder::with_queue(vec![
        tool_call_turn("fetch_secrets", "call_1", serde_json::json!({})),
        text_turn(FINAL_REPLY),
    ]));

    let executor = LeakyToolExecutor::new(LEAKY_CONTENT);

    let sanitizing_gate: Arc<dyn dasclaw_core::hooks::EgressGate> =
        Arc::new(SanitizingDisplayGate::new("sk-", SANITIZED_CONTENT));
    let recording_gate = RecordingEgressGate::wrap(sanitizing_gate);
    let calls_log = recording_gate.calls();

    let hooks = HookBundle {
        egress: Arc::new(recording_gate),
        ..HookBundle::noop()
    };

    let agent = Agent::builder()
        .responder_arc(responder.clone())
        .tool_executor(executor)
        .hooks(hooks)
        .build()
        .expect("build agent");

    let reply = agent
        .invoke("dump env", AgentRunOptions::invoke())
        .await
        .expect("agent.invoke should succeed")
        .text;
    assert_eq!(
        reply, FINAL_REPLY,
        "final reply should match scripted turn 2"
    );

    // --- Assertion 1: second LLM turn's ctx.messages tool_result is redacted.
    let snapshot = responder
        .snapshot(1)
        .expect("responder should have been called for the second turn");
    let tool_result_msg = snapshot
        .iter()
        .find(|m| matches!(m.role, Role::Tool))
        .expect("second-turn ctx.messages must contain a Tool message");

    assert!(
        !tool_result_msg.content.contains(RAW_SECRET),
        "raw secret must not survive into next LLM turn; got: {:?}",
        tool_result_msg.content,
    );
    assert!(
        tool_result_msg.content.contains(REDACTED_MARKER),
        "tool_result content must contain [REDACTED] marker; got: {:?}",
        tool_result_msg.content,
    );

    // --- Assertion 2: at least one UserDisplay check resulted in Redact.
    let log = calls_log.lock().expect("calls_log mutex not poisoned");
    let user_display_redactions = log
        .iter()
        .filter(|(kind, _payload, decision)| {
            matches!(kind, EgressKind::UserDisplay)
                && matches!(decision, EgressDecision::Redact { .. })
        })
        .count();
    assert!(
        user_display_redactions >= 1,
        "expected ≥1 UserDisplay Redact decisions; recorded calls: {:?}",
        *log,
    );

    // --- Assertion 3: final reply is clean (defensive — text_turn is direct).
    assert!(
        !reply.contains(RAW_SECRET),
        "final reply must not echo raw secret; got: {reply:?}"
    );
}
