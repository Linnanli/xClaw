//! ADR-153 §1.1 P0 e8 (A2 — L7 tool-arg redaction).
//!
//! Setup: scripted responder emits a tool call carrying an `authorization:
//! Bearer …` token in its arguments. The agent is wired with a
//! [`BlockingToolExecGate`] that returns [`EgressDecision::Block`] for any
//! `EgressKind::ToolExecution` and a [`RecordingToolExecutor`] that would
//! capture the call if it were ever invoked.
//!
//! Contract verified (matches `tests/safety_redaction_in_tool_args_e2e.rs`
//! row in ADR-153 §1.1 line 74):
//!   * The egress gate fires at `EgressKind::ToolExecution { tool }`.
//!   * The executor is **never** called (`call_count() == 0`).
//!   * The model receives an `Error: egress gate blocked …` tool_result so
//!     it can adapt — it does, by replying with a plain-text turn that the
//!     loop returns as the final answer.

use std::sync::Arc;

use dasclaw_core::hooks::HookBundle;
use dasclaw_runtime::{Agent, AgentRunOptions};

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{
    BlockingToolExecGate, RecordingEgressGate, RecordingToolExecutor, ScriptedResponder, text_turn,
    tool_call_turn,
};

#[tokio::test]
async fn req_dasclaw_cli_safety_e8_tool_arg_block_skips_executor() {
    // Turn 1: model emits a tool call leaking a bearer token in args.
    // Turn 2: after seeing the Error tool_result, model produces a final reply.
    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "fetch_url",
            "call_1",
            serde_json::json!({
                "url": "https://example.test/api",
                "authorization": "Bearer sk-leaked-token-shouldnt-egress"
            }),
        ),
        text_turn("declined: credential redacted"),
    ]);

    let executor = RecordingToolExecutor::new();
    let executor_handle = Arc::new(executor);

    let gate = RecordingEgressGate::wrap(Arc::new(BlockingToolExecGate::new(
        "L7 tool-arg redaction policy",
    )));
    let calls_log = gate.calls();

    let hooks = HookBundle {
        egress: Arc::new(gate),
        ..HookBundle::noop()
    };

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor_arc(executor_handle.clone())
        .hooks(hooks)
        .build()
        .expect("build agent");

    let reply = agent
        .invoke("please fetch", AgentRunOptions::invoke())
        .await
        .expect("run")
        .text;

    assert_eq!(
        executor_handle.call_count(),
        0,
        "executor must not run when tool-arg egress gate blocks"
    );
    assert_eq!(reply, "declined: credential redacted");

    let observed = calls_log.lock().expect("lock");
    let saw_tool_exec = observed.iter().any(|(kind, _payload, _decision)| {
        matches!(
            kind,
            dasclaw_core::hooks::EgressKind::ToolExecution { tool } if tool == "fetch_url"
        )
    });
    assert!(
        saw_tool_exec,
        "expected at least one EgressKind::ToolExecution observation, got {observed:?}"
    );
}
