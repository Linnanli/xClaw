//! ADR-153 §2.2 case **e26 (B10)** — D1+D3 跨 transport MCP merge e2e.
//!
//! Loads one stdio MCP server + one HTTP MCP server from a single
//! `--mcp-config`, then drives a scripted agent loop that calls **both**
//! sides via [`dasclaw_cli::run_with_tools_and_hooks`]. The invariants
//! pinned here are:
//!
//! 1. `McpToolExecutor::from_clients(...)` aggregates **both** transports'
//!    tool definitions under `<server>_<tool>` qualified names, with no
//!    namespace collision.
//! 2. Inside the agent loop, the model can address either side by its
//!    qualified name and the dispatcher routes to the right transport
//!    (stdio fixture for `stdio_srv_echo`, wiremock HTTP for
//!    `http_srv_echo`).
//! 3. The agent loop terminates normally after both tools have been
//!    invoked (no namespace-induced "unknown tool" error).
//!
//! Companion to:
//! - [`tests/mcp_multi_e2e.rs`] — two-stdio merge (e3/e4).
//! - [`tests/mcp_http_e2e.rs`] — single HTTP transport (e5/e6).
//! - [`tests/agent_loop_*`] — agent-loop scaffolding (e17/e18/e21/e23).
//!
//! This test adds the missing diagonal: **agent loop × mixed transports**.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dasclaw_cli::mcp::load_executor;
use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ToolCall, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::ToolExecutor;
use serde_json::{Value, json};
use tempfile::tempdir;
use wiremock::matchers::{body_partial_json, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "fixtures/agent_loop_fixtures.rs"]
mod agent_loop_fixtures;
use agent_loop_fixtures::{ScriptedResponder, text_turn, tool_call_turn};

fn stdio_fixture_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_stdio_echo_mcp_fixture"))
}

fn write_mixed_config(dir: &Path, stdio_cmd: &str, http_url: &str) -> PathBuf {
    let path = dir.join("mcp.json");
    let body = json!({
        "servers": [
            { "name": "stdio_srv", "command": stdio_cmd, "args": [], "env": {} },
            { "name": "http_srv",  "url": http_url, "headers": {} }
        ]
    });
    std::fs::write(&path, body.to_string()).expect("write config");
    path
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

/// Mount handshake + tools/list + tools/call on the wiremock HTTP MCP
/// fixture. Mirrors [`tests/mcp_http_e2e.rs::mount_handshake`] but adds
/// a fixed `tools/call` response so we can validate the merged dispatch.
async fn mount_http_mcp(server: &MockServer) {
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "initialize" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "http_mcp_merge_fixture", "version": "0.1.0" }
            }
        })))
        .mount(server)
        .await;

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
            "result": { "tools": [ echo_tool_def() ] }
        })))
        .mount(server)
        .await;

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
        .mount(server)
        .await;
}

/// Wrapping [`ToolExecutor`] that records every dispatched tool name
/// before forwarding to the inner executor.
///
/// The agent loop signature consumes the executor by value, so we cannot
/// inspect the inner `McpToolExecutor` afterwards; this thin recorder
/// captures the routing decisions on the way through.
struct InspectingToolExecutor<E: ToolExecutor> {
    inner: E,
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl<E: ToolExecutor> ToolExecutor for InspectingToolExecutor<E> {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        self.calls
            .lock()
            .expect("inspector mutex")
            .push(call.name.clone());
        self.inner.execute(call).await
    }
}

#[tokio::test]
async fn req_dasclaw_cli_agent_b10_e26_stdio_and_http_mcp_merge() {
    // ---- arrange: HTTP fixture ----
    let http_server = MockServer::start().await;
    mount_http_mcp(&http_server).await;

    // ---- arrange: mixed config (stdio + HTTP) ----
    let dir = tempdir().expect("tempdir");
    let stdio_bin = stdio_fixture_bin();
    let stdio_str = stdio_bin.to_str().expect("utf-8 stdio path");
    let config = write_mixed_config(dir.path(), stdio_str, &http_server.uri());

    // ---- act: load merged executor ----
    let mcp = load_executor(&config)
        .await
        .expect("load_executor must succeed with mixed stdio+http config");

    // Invariant 1: both transports surface their tool with qualified
    // names, no collision.
    let defs = mcp.definitions();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(
        names.contains(&"stdio_srv_echo"),
        "expected stdio_srv_echo in merged defs, got {names:?}"
    );
    assert!(
        names.contains(&"http_srv_echo"),
        "expected http_srv_echo in merged defs, got {names:?}"
    );
    assert_eq!(
        defs.len(),
        2,
        "expected exactly two merged tool defs, got {names:?}"
    );

    // ---- act: drive a scripted agent loop that calls both sides ----
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let inspector = InspectingToolExecutor {
        inner: mcp,
        calls: calls.clone(),
    };

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn("stdio_srv_echo", "c1", json!({ "message": "hello-stdio" })),
        tool_call_turn("http_srv_echo", "c2", json!({ "message": "hello-http" })),
        text_turn("done"),
    ]);

    let out = run_with_tools_and_hooks(
        responder,
        inspector,
        defs,
        HookBundle::noop(),
        "system",
        "please call both MCP servers",
    )
    .await
    .expect("agent loop should terminate normally across merged transports");

    // ---- assert: agent terminated on the final text turn ----
    assert_eq!(
        out, "done",
        "agent loop must return the final scripted text turn"
    );

    // Invariant 2 + 3: both qualified names were dispatched exactly
    // once, in scripted order.
    let observed = calls.lock().expect("inspector mutex").clone();
    assert_eq!(
        observed,
        vec!["stdio_srv_echo".to_string(), "http_srv_echo".to_string(),],
        "merged dispatcher must route both qualified names in order, got {observed:?}"
    );

    // Cross-check: the HTTP side actually received its tools/call (not
    // just answered list).
    let received = http_server
        .received_requests()
        .await
        .expect("wiremock should expose received requests");
    let http_tool_calls = received
        .iter()
        .filter(|r| {
            std::str::from_utf8(&r.body)
                .map(|s| s.contains("\"method\":\"tools/call\""))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        http_tool_calls, 1,
        "HTTP MCP server must see exactly one tools/call (got {http_tool_calls})"
    );
}
