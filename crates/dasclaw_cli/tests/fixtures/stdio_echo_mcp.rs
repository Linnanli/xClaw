//! Stdio MCP fixture for `dasclaw_cli` integration tests.
//!
//! Speaks the minimum subset of MCP needed to exercise
//! [`dasclaw_cli::mcp::load_executor`] end-to-end:
//!
//! - `initialize`               → static capability response
//! - `notifications/initialized`→ ignored (no `id`)
//! - `tools/list`               → declares a single `echo` tool
//! - `tools/call` for `echo`    → echoes the `message` arg back as text
//! - any other method           → JSON-RPC `-32601` method-not-found
//!
//! This binary is path-mapped into `tests/fixtures/` so the
//! `check_no_panics` guard treats it as test-only code (per
//! `scripts/check_no_panics.py::is_test_only_file`).

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            // Drop garbage; client will time out and surface the error.
            continue;
        };
        // Notifications have no `id`; they are fire-and-forget.
        let Some(id) = req.get("id").cloned() else {
            continue;
        };
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let response = build_response(id, method, req.get("params"));
        if writeln!(out, "{response}").is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
    }
}

fn build_response(id: Value, method: &str, params: Option<&Value>) -> Value {
    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "stdio_echo_mcp_fixture",
                    "version": "0.1.0"
                }
            }
        }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "echo",
                        "description": "Echoes its `message` argument back verbatim.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "message": { "type": "string" }
                            },
                            "required": ["message"]
                        }
                    }
                ]
            }
        }),
        "tools/call" => {
            let message = params
                .and_then(|p| p.get("arguments"))
                .and_then(|a| a.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("");
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [
                        { "type": "text", "text": message }
                    ],
                    "isError": false
                }
            })
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("method '{method}' not implemented in fixture")
            }
        }),
    }
}
