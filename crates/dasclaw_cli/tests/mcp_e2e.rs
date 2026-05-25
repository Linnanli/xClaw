//! End-to-end integration test for ADR-153 §4.4.5 (step 8): the CLI's
//! `--mcp-config` plumbing actually drives a real stdio MCP server
//! through `initialize` → `tools/list` → `tools/call`.
//!
//! Wires `dasclaw_cli::mcp::load_executor` against the
//! `stdio_echo_mcp_fixture` binary built from
//! [`tests/fixtures/stdio_echo_mcp.rs`].

use std::path::PathBuf;

use dasclaw_cli::mcp::load_executor;
use dasclaw_core::messages::ToolCall;
use dasclaw_runtime::ToolExecutor;
use serde_json::json;
use tempfile::tempdir;

/// Path to the fixture MCP server binary, injected by cargo when this
/// integration test is built.
fn fixture_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_stdio_echo_mcp_fixture"))
}

fn write_config(dir: &std::path::Path, command: &str) -> PathBuf {
    let path = dir.join("mcp.json");
    let body = json!({
        "servers": [
            {
                "name": "fixture",
                "command": command,
                "args": [],
                "env": {}
            }
        ]
    });
    std::fs::write(&path, body.to_string()).expect("write config");
    path
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e1_end_to_end_loads_tool_and_dispatches_call() {
    let dir = tempdir().expect("tempdir");
    let config = write_config(dir.path(), fixture_bin().to_str().expect("utf-8 path"));

    let executor = load_executor(&config).await.expect("load_executor");

    // tools/list should have surfaced exactly the one echo tool, qualified.
    let defs = executor.definitions();
    assert_eq!(defs.len(), 1, "expected exactly one tool, got {defs:?}");
    let def = &defs[0];
    assert_eq!(def.name, "fixture_echo");
    assert!(
        def.description.contains("Echoes"),
        "description should match fixture, got {:?}",
        def.description
    );

    // tools/call should round-trip the `message` arg back as text content.
    let call = ToolCall {
        id: "call-1".into(),
        name: "fixture_echo".into(),
        arguments: json!({ "message": "hello from e2e" }),
        reasoning: None,
    };
    let result = executor.execute(&call).await.expect("execute");
    assert!(!result.is_error, "tool call failed: {}", result.content);
    assert_eq!(result.tool_call_id, "call-1");
    assert_eq!(result.name, "fixture_echo");
    assert_eq!(result.content, "hello from e2e");
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e2_unknown_qualified_tool_is_tool_error_not_panic() {
    let dir = tempdir().expect("tempdir");
    let config = write_config(dir.path(), fixture_bin().to_str().expect("utf-8 path"));
    let executor = load_executor(&config).await.expect("load_executor");

    let call = ToolCall {
        id: "call-2".into(),
        name: "fixture_does_not_exist".into(),
        arguments: json!({}),
        reasoning: None,
    };
    let result = executor.execute(&call).await.expect("execute");
    assert!(result.is_error, "unknown tool must surface is_error=true");
    assert!(
        result.content.contains("unknown MCP tool"),
        "error message should mention unknown MCP tool, got: {}",
        result.content
    );
}
