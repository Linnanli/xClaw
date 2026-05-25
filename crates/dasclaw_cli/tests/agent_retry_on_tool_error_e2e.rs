//! ADR-153 §1.1 B2 (e18): retry on tool error. First `read_file` returns
//! `is_error=true` with an ENOENT-shaped payload; the model is scripted
//! to react by calling `read_file` again with a different path. The
//! loop must feed the error result back to the model rather than
//! propagating it as a hard failure, and the second attempt must
//! observe the `success_content` payload.

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use dasclaw_runtime::Agent;
use serde_json::json;

use fixtures::{FlakyToolExecutor, ScriptedResponder, no_tool_defs, text_turn, tool_call_turn};

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
