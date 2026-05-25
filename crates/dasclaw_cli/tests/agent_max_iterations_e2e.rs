//! ADR-153 §1.1 B5 (e21): `max_iterations` is an exact, observable cap.
//! With `max_iterations = 3` and a responder that always asks for one
//! more tool call, the loop must exit with
//! `AgentError::MaxIterations(3)` after exactly 3 tool dispatches —
//! not 2 (off-by-one early), not 4 (off-by-one late).

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use dasclaw_core::agentic_loop::AgenticLoopConfig;
use dasclaw_runtime::{Agent, AgentError};
use serde_json::json;

use fixtures::{
    RecordingToolExecutor, ScriptedResponder, no_tool_defs, tool_call_turn,
};

#[tokio::test]
async fn req_dasclaw_cli_loop_e21_max_iterations_3_exact_count() {
    // Queue more turns than the cap so a successful early exit would
    // visibly leave headroom in the queue. Every turn is a tool call so
    // the loop never reaches a terminal text state on its own.
    let script = (0..10)
        .map(|i| {
            tool_call_turn(
                "noop",
                format!("call_noop_{i}"),
                json!({ "iteration": i }),
            )
        })
        .collect();
    let responder = ScriptedResponder::with_queue(script);

    let executor = RecordingToolExecutor::new().with_reply("noop", "ok");
    let calls = executor.calls();

    let cfg = AgenticLoopConfig {
        max_iterations: 3,
        ..AgenticLoopConfig::default()
    };

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .loop_config(cfg)
        .build()
        .expect("agent builds");

    let err = agent
        .run("spin forever")
        .await
        .expect_err("looped responder must trip the cap");

    match err {
        AgentError::MaxIterations(n) => {
            assert_eq!(n, 3, "AgentError::MaxIterations must echo the configured cap");
        }
        other => panic!("expected MaxIterations(3), got {other:?}"),
    }

    let count = calls.lock().expect("calls log").len();
    assert_eq!(
        count, 3,
        "exactly max_iterations tool dispatches must have run, got {count}"
    );
}
