//! End-to-end integration test for ADR-153 §4.4.5 (step 8): when the CLI
//! loads multiple stdio MCP servers from a single config, each server's
//! tools are namespaced as `<server_name>_<tool>` so two servers that
//! happen to expose the same underlying tool name (`echo`) coexist
//! without collision and each call routes to the right subprocess.
//!
//! Uses the same `stdio_echo_mcp_fixture` binary built from
//! [`tests/fixtures/stdio_echo_mcp.rs`] for both servers; the only
//! difference between them is the logical `name` in the config.

use std::path::PathBuf;

use dasclaw_cli::mcp::load_executor;
use dasclaw_core::messages::ToolCall;
use dasclaw_runtime::ToolExecutor;
use serde_json::json;
use tempfile::tempdir;

fn fixture_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_stdio_echo_mcp_fixture"))
}

fn write_two_server_config(dir: &std::path::Path, command: &str) -> PathBuf {
    let path = dir.join("mcp.json");
    let body = json!({
        "servers": [
            { "name": "srv_a", "command": command, "args": [], "env": {} },
            { "name": "srv_b", "command": command, "args": [], "env": {} }
        ]
    });
    std::fs::write(&path, body.to_string()).expect("write config");
    path
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e3_two_servers_namespace_disjointly() {
    let dir = tempdir().expect("tempdir");
    let bin = fixture_bin();
    let bin_str = bin.to_str().expect("utf-8 path");
    let config = write_two_server_config(dir.path(), bin_str);

    let executor = load_executor(&config).await.expect("load_executor");
    let defs = executor.definitions();

    // Both servers expose `echo`, qualified by server name. Order
    // follows the config order.
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["srv_a_echo", "srv_b_echo"],
        "expected two namespaced tool defs, got {names:?}"
    );

    // Dispatch hits the right subprocess: each call should round-trip its
    // own message verbatim, and the response is tagged with the
    // qualified name (not the underlying `echo`).
    let call_a = ToolCall {
        id: "a-1".into(),
        name: "srv_a_echo".into(),
        arguments: json!({ "message": "from A" }),
        reasoning: None,
    };
    let result_a = executor.execute(&call_a).await.expect("execute A");
    assert!(!result_a.is_error, "A failed: {}", result_a.content);
    assert_eq!(result_a.name, "srv_a_echo");
    assert_eq!(result_a.tool_call_id, "a-1");
    assert_eq!(result_a.content, "from A");

    let call_b = ToolCall {
        id: "b-1".into(),
        name: "srv_b_echo".into(),
        arguments: json!({ "message": "from B" }),
        reasoning: None,
    };
    let result_b = executor.execute(&call_b).await.expect("execute B");
    assert!(!result_b.is_error, "B failed: {}", result_b.content);
    assert_eq!(result_b.name, "srv_b_echo");
    assert_eq!(result_b.tool_call_id, "b-1");
    assert_eq!(result_b.content, "from B");
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e4_call_to_other_servers_unqualified_name_is_tool_error() {
    // Routing must require the full `<server>_<tool>` qualified name.
    // Calling the bare underlying `echo` (no prefix) when two servers
    // both publish `echo` must surface a tool error rather than
    // ambiguously picking one subprocess.
    let dir = tempdir().expect("tempdir");
    let bin = fixture_bin();
    let bin_str = bin.to_str().expect("utf-8 path");
    let config = write_two_server_config(dir.path(), bin_str);

    let executor = load_executor(&config).await.expect("load_executor");

    let call = ToolCall {
        id: "x-1".into(),
        name: "echo".into(), // unqualified — must NOT route
        arguments: json!({ "message": "unrouted" }),
        reasoning: None,
    };
    let result = executor.execute(&call).await.expect("execute");
    assert!(
        result.is_error,
        "unqualified call must fail, got success: {}",
        result.content
    );
    assert!(
        result.content.contains("unknown MCP tool"),
        "error message should mention unknown MCP tool, got: {}",
        result.content
    );
}
