//! ADR-153 §1.1 B2 (e18) + B3 (e19): retry / max-iterations cap on tool error.
//!
//! - **e18**: First `read_file` returns `is_error=true` with an ENOENT-shaped
//!   payload; the model is scripted to react by calling `read_file` again
//!   with a different path. The loop must feed the error result back to the
//!   model rather than propagating it as a hard failure, and the second
//!   attempt must observe the `success_content` payload.
//! - **e19**: Tool keeps failing forever; the model also keeps retrying. The
//!   loop must terminate at `max_iterations = 5` with
//!   `AgentError::MaxIterations(5)` instead of looping forever.

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use dasclaw_core::agentic_loop::AgenticLoopConfig;
use dasclaw_runtime::{Agent, AgentError};
use serde_json::json;

use fixtures::{
    AlwaysFailingToolExecutor, FlakyToolExecutor, ScriptedResponder, no_tool_defs, text_turn,
    tool_call_turn,
};

#[tokio::test]
async fn req_dasclaw_cli_loop_e18_retry_recovers_after_enoent() {
    // T1: model asks for a path that the (flaky) executor reports as
    // missing. T2: model retries with a different path. T3: final text
    // cites the recovered content.
    let script = vec![
        tool_call_turn(
            "read_file",
            "call_read_bad",
            json!({ "path": "missing.txt" }),
        ),
        tool_call_turn(
            "read_file",
            "call_read_good",
            json!({ "path": "exists.txt" }),
        ),
        text_turn("recovered: hello-from-disk"),
    ];
    let responder = ScriptedResponder::with_queue(script);

    let executor = FlakyToolExecutor::new(
        ["read_file"],
        "ENOENT: no such file 'missing.txt'",
        "hello-from-disk",
    );
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .build()
        .expect("agent builds");

    let reply = agent
        .run("read the report file")
        .await
        .expect("retry path must surface as Ok(...) from the loop");

    assert!(
        reply.contains("hello-from-disk"),
        "final reply must cite the recovered content, got: {reply:?}"
    );

    let log = calls.lock().expect("calls log");
    assert_eq!(
        log.len(),
        2,
        "expected exactly 2 read_file invocations (1 error + 1 success), got: {log:#?}"
    );
    assert_eq!(
        log[0].arguments,
        json!({ "path": "missing.txt" }),
        "first attempt must hit the bad path"
    );
    assert_eq!(
        log[1].arguments,
        json!({ "path": "exists.txt" }),
        "retry must use the model's corrected path"
    );
}

// ADR-153 §1.1 B3 (e19): when a tool keeps failing and the scripted
// model keeps retrying, the loop must terminate at `max_iterations`
// instead of looping forever. We set `max_iterations = 5` explicitly
// (matrix value), queue more turns than the cap, and assert both the
// echoed cap inside `AgentError::MaxIterations(n)` and the exact tool
// dispatch count (n).
#[tokio::test]
async fn req_dasclaw_cli_loop_e19_max_iterations_5_with_permanent_failure() {
    let script = (0..10)
        .map(|i| {
            tool_call_turn(
                "read_file",
                format!("call_read_retry_{i}"),
                json!({ "path": format!("attempt_{i}.txt") }),
            )
        })
        // A trailing text_turn would be unreachable: the cap fires
        // before the model gets a chance to give up. Keeping the queue
        // tool-only makes the failure mode unambiguous.
        .collect();
    let responder = ScriptedResponder::with_queue(script);

    let executor = AlwaysFailingToolExecutor::new("ENOENT: target never exists");
    let calls = executor.calls();

    let cfg = AgenticLoopConfig {
        max_iterations: 5,
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
        .run("keep retrying the broken file")
        .await
        .expect_err("permanent failure must reach the cap, not silently succeed");

    match err {
        AgentError::MaxIterations(n) => {
            assert_eq!(
                n, 5,
                "AgentError::MaxIterations must echo the configured cap (5)"
            );
        }
        other => panic!("expected MaxIterations(5), got {other:?}"),
    }

    let count = calls.lock().expect("always-failing calls log").len();
    assert_eq!(
        count, 5,
        "permanent-failure retries must dispatch exactly max_iterations tool calls, got {count}"
    );
}
