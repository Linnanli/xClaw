//! Minimal WASM component fixture for `dasclaw_wasm_tools` capability opt-in tests.
//!
//! Implements the `sandboxed-tool` world from `wit/tool.wit`. The fixture
//! dispatches on a minimal `"action"` substring in `req.params`:
//!
//! | action substring   | host import probed | default-deny behaviour locked            |
//! |--------------------|--------------------|------------------------------------------|
//! | (none / `http`)    | `http-request`     | host returns `"HTTP capability not …"`   |
//! | `workspace`        | `workspace-read`   | host returns `None` (no value crosses)   |
//! | `secrets`          | `secret-exists`    | host returns `false` (existence denied)  |
//! | `rate_limit`       | `http-request` ×N  | host returns rate-limit error after cap  |
//!
//! Parsing is intentionally a substring check, not full JSON parsing, to
//! keep the produced wasm artifact small and avoid pulling in serde at the
//! fixture boundary. Tests that mis-spell the action fall through to the
//! default `http` branch, which is the original `#854` behaviour.

wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "../../../wit/tool.wit",
});

use exports::near::agent::tool::{Guest, Request, Response};
use near::agent::host;

struct Component;

fn run_http() -> Response {
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

fn run_workspace() -> Response {
    // `workspace-read` returns `option<string>`. Without the capability the
    // host returns `None` (see `HostState::workspace_read`); the fixture
    // reports that observation back as an error so the test can assert the
    // "no data crossed the boundary" contract.
    match host::workspace_read("test.txt") {
        Some(value) => Response {
            output: Some(format!(r#"{{"workspace_read":{value:?}}}"#)),
            error: None,
        },
        None => Response {
            output: None,
            error: Some("workspace-read returned None (capability denied)".to_string()),
        },
    }
}

fn run_secrets() -> Response {
    // `secret-exists` returns `bool`. Without the capability the host
    // returns `false` (see `HostState::secret_exists`).
    if host::secret_exists("ANY_KEY") {
        Response {
            output: Some(r#"{"secret_exists":true}"#.to_string()),
            error: None,
        }
    } else {
        Response {
            output: None,
            error: Some("secret-exists returned false (capability denied)".to_string()),
        }
    }
}

fn run_rate_limit() -> Response {
    // Loop the host `http-request` import until the per-execution rate
    // limit trips. Uses a TEST-NET literal IP so `reject_private_ip` skips
    // DNS resolution; the test grants an `http` capability allowlist + an
    // `HttpInterceptor` that short-circuits with a canned 200, so the
    // first 50 calls succeed and the 51st surfaces the rate-limit error
    // verbatim. `MAX_REQUESTS_PER_EXECUTION = 50` lives in `host.rs`.
    const URL: &str = "https://203.0.113.1/";
    const ATTEMPTS: usize = 60;
    let mut last_err = String::from("no calls attempted");
    let mut succeeded = 0usize;
    for i in 0..ATTEMPTS {
        match host::http_request("GET", URL, "{}", None, None) {
            Ok(_) => succeeded += 1,
            Err(msg) => {
                last_err = format!("call {i}: {msg}");
                break;
            }
        }
    }
    Response {
        output: Some(format!(r#"{{"succeeded":{succeeded}}}"#)),
        error: Some(last_err),
    }
}

impl Guest for Component {
    fn execute(req: Request) -> Response {
        let p = req.params.as_str();
        if p.contains("\"workspace\"") {
            run_workspace()
        } else if p.contains("\"secrets\"") {
            run_secrets()
        } else if p.contains("\"rate_limit\"") {
            run_rate_limit()
        } else {
            run_http()
        }
    }

    fn schema() -> String {
        r#"{"type":"object","properties":{"action":{"type":"string","enum":["http","workspace","secrets","rate_limit"]}},"additionalProperties":true}"#.to_string()
    }

    fn description() -> String {
        "Minimal capability fixture (http/workspace/secrets/rate_limit) for opt-in tests".to_string()
    }
}

export!(Component);
