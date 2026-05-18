//! Contract tests for `SandboxExecRequest.network`.
//!
//! These tests pin the IPC-shape guarantees the `dasclaw_core` hook
//! contract makes for sandbox runtime integrations (Wave-C2b sibling of
//! `dasclaw_sandbox::SandboxExecRequest.network`, but on the
//! dependency-free hook seam):
//!
//! 1. **Fail-Safe default** — Constructing a request without an explicit
//!    `network` value (via Serde, mimicking an NDJSON peer that predates
//!    this field) yields `network == None`, i.e. no egress.
//! 2. **Round-trip stability** — `Some(SandboxNetworkHint { .. })`
//!    serializes & deserializes losslessly, so future-runtime daemons can
//!    safely thread proxy endpoints across the JSON boundary.
//!
//! These tests deliberately live as an integration test (not a unit
//! test) so they exercise only the public `dasclaw_core` API surface and
//! catch any accidental visibility regressions on `SandboxNetworkHint`.

use std::collections::HashMap;
use std::path::PathBuf;

use dasclaw_core::{SandboxExecRequest, SandboxNetworkHint};

#[test]
fn req_sandbox_exec_request_defaults_network_to_none_on_legacy_payload() {
    // Legacy NDJSON peer: no `network` key present.
    let legacy = r#"{
        "command": "ls",
        "cwd": "/",
        "env": {}
    }"#;
    let req: SandboxExecRequest =
        serde_json::from_str(legacy).expect("legacy payload must deserialize");
    assert!(
        req.network.is_none(),
        "Fail-Safe: missing `network` key must decode to None (no egress)"
    );
}

#[test]
fn req_sandbox_exec_request_network_round_trip_preserves_proxy_url() {
    let original = SandboxExecRequest {
        command: "curl https://example.test".to_string(),
        cwd: PathBuf::from("/workspace"),
        env: HashMap::new(),
        network: Some(SandboxNetworkHint {
            proxy_url: "http://127.0.0.1:48273".to_string(),
        }),
    };

    let wire = serde_json::to_string(&original).expect("serialize");
    let decoded: SandboxExecRequest = serde_json::from_str(&wire).expect("deserialize");

    let hint = decoded
        .network
        .as_ref()
        .expect("network must survive round-trip");
    assert_eq!(hint.proxy_url, "http://127.0.0.1:48273");
}
