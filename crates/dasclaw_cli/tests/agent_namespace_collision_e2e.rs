//! ADR-153 §1.1 B4 (e20): namespace collision across two simulated MCP
//! servers. Both servers expose a tool conceptually called `search`,
//! exposed via the qualified names `a_search` and `b_search`. The
//! scripted model only ever calls `b_search`; the loop must route
//! exactly there, never invoking `a_search`. We verify this with a
//! single `RecordingToolExecutor` that owns both qualified names — the
//! executor's call log is the authoritative routing record.

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;

use dasclaw_runtime::{Agent, AgentRunOptions};
use serde_json::json;

use fixtures::{RecordingToolExecutor, ScriptedResponder, no_tool_defs, text_turn, tool_call_turn};

#[tokio::test]
async fn req_dasclaw_cli_loop_e20_namespace_collision_routes_to_b() {
    // Two scripted turns:
    //   T1: model picks the b-side search explicitly.
    //   T2: model surfaces a final answer citing b's content.
    // A would-be ambiguous "search" tool is never named, so a correct
    // implementation must not silently pick a fallback.
    let script = vec![
        tool_call_turn(
            "b_search",
            "call_b_search_1",
            json!({ "q": "release notes" }),
        ),
        text_turn("found in b: b-result-payload"),
    ];
    let responder = ScriptedResponder::with_queue(script);

    let executor = RecordingToolExecutor::new()
        .with_reply("a_search", "a-result-payload")
        .with_reply("b_search", "b-result-payload");
    let calls = executor.calls();

    let agent = Agent::builder()
        .responder(responder)
        .tool_executor(executor)
        .tools(no_tool_defs())
        .build()
        .expect("agent builds");

    let reply = agent
        .invoke("look it up in b", AgentRunOptions::invoke())
        .await
        .expect("routing the b-side tool must succeed")
        .text;

    assert!(
        reply.contains("b-result-payload"),
        "final reply must cite the b-side payload, got: {reply:?}"
    );

    let log = calls.lock().expect("calls log");
    assert_eq!(
        log.len(),
        1,
        "exactly one tool dispatch must have happened, got: {log:#?}"
    );
    assert_eq!(
        log[0].name, "b_search",
        "routing must hit the qualified b-side name, not the a-side"
    );
    assert!(
        log.iter().all(|c| c.name != "a_search"),
        "a_search must never be invoked when the model only names b_search"
    );
}
