//! Minimal WASM component fixture for `dasclaw_wasm_tools` capability opt-in tests.
//!
//! Implements the `sandboxed-tool` world from `wit/tool.wit`. On `execute()`,
//! attempts a host HTTP request and returns the host's error string. When the
//! host has no `http` capability granted, the host returns
//! `"HTTP capability not granted"` (see `crates/dasclaw_wasm_tools/src/host.rs`
//! `HostState::check_http_allowed`).
//!
//! Used by `dasclaw_wasm_tools` self-tests and downstream `dasclaw_cli` A7 e2e.

wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "../../../wit/tool.wit",
});

use exports::near::agent::tool::{Guest, Request, Response};
use near::agent::host;

struct Component;

impl Guest for Component {
    fn execute(_req: Request) -> Response {
        // Attempt the simplest possible HTTP call. The host enforces capability
        // gating; without the `http` capability this returns Err immediately.
        match host::http_request("GET", "https://example.com/", "{}", None, None) {
            Ok(_) => Response {
                output: Some(r#"{"status":"ok"}"#.to_string()),
                error: None,
            },
            Err(msg) => Response {
                output: None,
                error: Some(msg),
            },
        }
    }

    fn schema() -> String {
        r#"{"type":"object","properties":{},"additionalProperties":true}"#.to_string()
    }

    fn description() -> String {
        "Minimal HTTP fixture for capability opt-in tests".to_string()
    }
}

export!(Component);
