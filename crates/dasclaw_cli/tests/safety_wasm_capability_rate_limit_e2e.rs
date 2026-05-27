//! ADR-153 §1.1 A7 follow-up — capability matrix: per-execution HTTP
//! rate-limit default-deny e2e (issue #869).
//!
//! Differs from the workspace / secrets rows in that the rate-limit gate
//! lives **inside** the `http` capability, so the test must:
//!
//! 1. Grant the `http` capability with an allowlist covering the fixture
//!    target URL (otherwise the allowlist check trips before rate limit
//!    and we'd measure the wrong gate).
//! 2. Install an `HttpInterceptor` that returns a canned 200 response,
//!    so the test never touches real network. (The interceptor sits
//!    **after** the rate-limit check inside the wrapper, so calls 1..50
//!    flow allowlist → rate-limit-record → leak-scan → IP-check → canned
//!    reply, and call 51 trips rate-limit before any of the later
//!    stages.)
//! 3. Use a public, literal IPv4 (TEST-NET-3, RFC 5737) so the
//!    `reject_private_ip` step skips DNS resolution entirely.
//!
//! The fixture loops up to 60 calls and breaks on the first host error;
//! both the agent-loop test and the direct ToolExecutor test pin the
//! verbatim host gate string `"Too many HTTP requests in single
//! execution (max 50)"` and the wrapper's `"Rate limit exceeded: "`
//! prefix so a future change to either layer surfaces immediately.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::ToolCall;
use dasclaw_runtime::recording::{HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor};
use dasclaw_wasm_tools::{Capabilities, EndpointPattern, HttpCapability};
use serde_json::json;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod agent_loop_fixtures;
#[path = "fixtures/wasm_capability_helpers.rs"]
mod wasm_capability_helpers;

use agent_loop_fixtures::{ScriptedResponder, text_turn, tool_call_turn};
use wasm_capability_helpers::{CapWasmExecutor, fixture_tool_def};

/// Interceptor that short-circuits every outbound request with a canned
/// 200 OK reply. Lets the rate-limit test exercise the gate without
/// touching real network.
#[derive(Debug, Default)]
struct CannedOkInterceptor;

#[async_trait]
impl HttpInterceptor for CannedOkInterceptor {
    async fn before_request(&self, _req: &HttpExchangeRequest) -> Option<HttpExchangeResponse> {
        Some(HttpExchangeResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: r#"{"ok":true}"#.to_string(),
        })
    }

    async fn after_response(&self, _req: &HttpExchangeRequest, _resp: &HttpExchangeResponse) {}
}

fn http_caps_for_test_net() -> Capabilities {
    let pattern = EndpointPattern::host("203.0.113.1");
    let http = HttpCapability::new(vec![pattern]);
    Capabilities::default().with_http(http)
}

#[tokio::test]
async fn req_dasclaw_cli_safety_a7_wasm_http_rate_limit_surfaces_as_tool_error() {
    let tool_name = "fixture_rate_limit";
    let interceptor: Arc<dyn HttpInterceptor> = Arc::new(CannedOkInterceptor);
    let executor =
        CapWasmExecutor::build(tool_name, http_caps_for_test_net(), Some(interceptor)).await;

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(tool_name, "call-rl", json!({ "action": "rate_limit" })),
        text_turn("acknowledged: rate limit tripped"),
    ]);

    let outcome = run_with_tools_and_hooks(
        responder,
        executor,
        vec![fixture_tool_def(tool_name)],
        HookBundle::noop(),
        "you are running inside a capability-gated CLI",
        "please call the fixture rate_limit tool",
    )
    .await;

    let reply = outcome.expect("agent loop must not crash on rate-limit denial");
    assert_eq!(reply, "acknowledged: rate limit tripped");
}

#[tokio::test]
async fn req_dasclaw_cli_safety_a7_wasm_rate_limit_tool_result_carries_host_gate_message() {
    let tool_name = "fixture_rate_limit";
    let interceptor: Arc<dyn HttpInterceptor> = Arc::new(CannedOkInterceptor);
    let executor =
        CapWasmExecutor::build(tool_name, http_caps_for_test_net(), Some(interceptor)).await;

    let call = ToolCall {
        id: "direct-call-rl".to_string(),
        name: tool_name.to_string(),
        arguments: json!({ "action": "rate_limit" }),
        reasoning: None,
    };

    let result = dasclaw_runtime::ToolExecutor::execute(&executor, &call)
        .await
        .expect("ToolExecutor must not raise HostError on rate-limit denial");

    assert!(
        result.is_error,
        "rate-limited tool must report is_error=true, got: {result:?}"
    );
    // The fixture surfaces the host error verbatim, prefixed by the
    // wrapper. Pin both the wrapper-layer prefix and the host-layer
    // message so regressions in either layer are caught.
    assert!(
        result.content.contains("Rate limit exceeded"),
        "tool result must carry the wrapper rate-limit prefix, got: {}",
        result.content
    );
    assert!(
        result
            .content
            .contains("Too many HTTP requests in single execution (max 50)"),
        "tool result must carry the host gate string verbatim, got: {}",
        result.content
    );
    // The fixture reports which iteration tripped the gate. After 50
    // successful canned replies the 51st call (index 50) must fail.
    assert!(
        result.content.contains("call 50:"),
        "rate limit must trip on the 51st call (index 50), got: {}",
        result.content
    );
}
