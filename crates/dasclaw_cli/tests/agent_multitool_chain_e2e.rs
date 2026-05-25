//! ADR-153 §1.1 B1 (e17): multi-tool chain — three LLM turns drive two
//! distinct tool calls, then a final text turn that cites both tool
//! outputs. Verifies the loop genuinely round-trips tool_result back
//! into the model and doesn't short-circuit on the first turn.
//!
//! Fixtures live in a dedicated file (`agent_loop_fixtures.rs`) to keep
//! zero file overlap with the W6.2 safety-stack tests.

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use dasclaw_runtime::Agent;
use serde_json::json;

use fixtures::{
    RecordingToolExecutor, ScriptedResponder, no_tool_defs, text_turn, tool_call_turn,
};

#[tokio::test]
async fn req_dasclaw_cli_loop_e17_multitool_chain_completes() {
    // Three LLM turns: list_files → read_file → final text.
    let script = vec![
        tool_call_turn("list_files", "call_list_1", json!({ "path": "." })),
        tool_call_turn(
            "read_file",
            "call_read_1",
            json!({ "path": "fileA.txt" }),
        ),
        text_turn("done: listed=[fileA.txt,fileB.txt], read=contents-of-fileA"),
    ];
    let responder = ScriptedResponder::with_queue(script);

    let executor = RecordingToolExecutor::new()
        .with_reply("list_files", "fileA.txt\nfileB.txt")
        .with_reply("read_file", "contents-of-fileA");
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .build()
        .expect("agent builds");

    let reply = agent
        .run("please list the dir and then read fileA")
        .await
        .expect("multi-tool chain succeeds");

    assert!(
        reply.contains("fileA.txt") && reply.contains("contents-of-fileA"),
        "final reply must cite evidence from both tool calls, got: {reply:?}"
    );

    let log = calls.lock().expect("calls log");
    assert_eq!(log.len(), 2, "expected exactly 2 tool calls, got: {log:#?}");
    assert_eq!(log[0].name, "list_files", "first call must be list_files");
    assert_eq!(log[1].name, "read_file", "second call must be read_file");
    assert_eq!(
        log[1].arguments,
        json!({ "path": "fileA.txt" }),
        "second call must forward the model's chosen path"
    );
}
