//! End-to-end integration test for ADR-153 §4.4.5 (step 8) HTTP branch:
//! verifies that `dasclaw_cli::mcp::load_executor` drives a real
//! `HttpMcpTransport` through `initialize` →
//! `notifications/initialized` → `tools/list` → `tools/call` against an
//! HTTP MCP server.
//!
//! Companion to [`tests/mcp_e2e.rs`] (stdio path) and
//! [`tests/mcp_multi_e2e.rs`] (multi-server namespace). The stdio tests
//! cover the subprocess fixture; this file covers the HTTP transport
//! path that those tests never touch.
//!
//! The HTTP MCP server is built from the existing `wiremock` dev-dep
//! (already used by [`tests/live_provider.rs`]) so no new dependency is
//! introduced.

use std::path::{Path, PathBuf};

use dasclaw_cli::mcp::load_executor;
use dasclaw_core::messages::ToolCall;
use dasclaw_runtime::ToolExecutor;
use serde_json::{Value, json};
use tempfile::tempdir;
use wiremock::matchers::{body_partial_json, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_http_config(dir: &Path, name: &str, url: &str) -> PathBuf {
    let path = dir.join("mcp.json");
    let body = json!({
        "servers": [
            { "name": name, "url": url, "headers": {} }
        ]
    });
    std::fs::write(&path, body.to_string()).expect("write config");
    path
}

/// Mount the initialize / initialized-notification / tools/list mocks
/// that every test in this file needs.  Returns the server so the caller
/// can mount additional `tools/call` behaviour on top.
async fn mount_handshake(server: &MockServer, tool_def: Value) {
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "initialize" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "http_echo_mcp_fixture", "version": "0.1.0" }
            }
        })))
        .mount(server)
        .await;

    // Per HttpMcpTransport regression test for #1436: notifications return 202.
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({ "method": "notifications/initialized" }),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "tools/list" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": { "tools": [ tool_def ] }
        })))
        .mount(server)
        .await;
}

fn echo_tool_def() -> Value {
    json!({
        "name": "echo",
        "description": "Echoes its `message` argument back verbatim.",
        "inputSchema": {
            "type": "object",
            "properties": { "message": { "type": "string" } },
            "required": ["message"]
        }
    })
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e5_http_transport_dispatches_tool_call() {
    let server = MockServer::start().await;
    mount_handshake(&server, echo_tool_def()).await;

    // tools/call returns a fixed echoed text content (the fixture does
    // not parse arguments — verifying that round-trip happens at all is
    // enough at this layer; argument plumbing is covered by the stdio
    // fixture in mcp_e2e.rs).
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "tools/call" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": {
                "content": [ { "type": "text", "text": "from-http" } ],
                "isError": false
            }
        })))
        .mount(&server)
        .await;

    let dir = tempdir().expect("tempdir");
    let config = write_http_config(dir.path(), "remote", &server.uri());

    let executor = load_executor(&config).await.expect("load_executor");

    // tools/list should have surfaced exactly the one echo tool,
    // qualified with the server name from the config (not the
    // serverInfo.name advertised by the MCP server).
    let defs = executor.definitions();
    assert_eq!(defs.len(), 1, "expected one tool, got {defs:?}");
    assert_eq!(defs[0].name, "remote_echo");
    assert!(
        defs[0].description.contains("Echoes"),
        "description should propagate from tool def, got {:?}",
        defs[0].description
    );

    let call = ToolCall {
        id: "h-1".into(),
        name: "remote_echo".into(),
        arguments: json!({ "message": "ignored by fixture" }),
        reasoning: None,
    };
    let result = executor.execute(&call).await.expect("execute");
    assert!(!result.is_error, "tool call failed: {}", result.content);
    assert_eq!(result.tool_call_id, "h-1");
    assert_eq!(result.name, "remote_echo");
    assert_eq!(result.content, "from-http");
}

#[tokio::test]
async fn req_dasclaw_cli_mcp_e6_http_tools_call_5xx_is_tool_error_not_panic() {
    // Handshake + list succeed, only tools/call returns 500. This
    // pins the contract in McpToolExecutor (docstring at
    // crates/dasclaw_mcp/src/executor.rs:15): "transport errors are
    // surfaced as ToolResult { is_error: true }" — so the agent loop
    // sees a recoverable tool error, not a propagated Err.
    let server = MockServer::start().await;
    mount_handshake(&server, echo_tool_def()).await;

    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "tools/call" })))
        .respond_with(ResponseTemplate::new(500).set_body_string("backend on fire"))
        .mount(&server)
        .await;

    let dir = tempdir().expect("tempdir");
    let config = write_http_config(dir.path(), "remote", &server.uri());
    let executor = load_executor(&config).await.expect("load_executor");

    let call = ToolCall {
        id: "h-2".into(),
        name: "remote_echo".into(),
        arguments: json!({ "message": "anything" }),
        reasoning: None,
    };
    let result = executor.execute(&call).await.expect(
        "execute must return Ok with is_error=true for transport failures, \
         not propagate the error",
    );
    assert!(
        result.is_error,
        "5xx on tools/call must surface is_error=true, got {result:?}"
    );
    assert_eq!(result.tool_call_id, "h-2");
    assert_eq!(result.name, "remote_echo");
}
