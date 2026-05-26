//! W6.6c — ADR-153 §1.1 A7 / e13: L7 WASM capability opt-in default-deny.
//!
//! Wires a real `wasm32-wasip2` component (compiled by `dasclaw_wasm_tools`
//! build.rs and exported as [`dasclaw_wasm_tools::NO_HTTP_CAP_WASM`]) whose
//! `execute` body calls the host `http-request` import. When the wrapper is
//! constructed with [`Capabilities::default()`] (no `http` granted), the host
//! gate at `dasclaw_wasm_tools::host` returns the literal
//! `"HTTP capability not granted"`. The agent loop must observe this as a
//! reported tool error (`is_error=true`), not as a host crash or a
//! `LoopFailure`.
//!
//! This test does **not** depend on `dasclaw_safety` and does **not** drive
//! the CLI binary — it injects the WASM wrapper through `ToolExecutor` at
//! the `run_with_tools_and_hooks` library seam, mirroring the A6 pattern
//! (#852) one slice up the stack. Per ADR-153 §3 R2, the assertion targets
//! the contract between the wasm host gate and the tool result envelope.
//!
//! ## Non-goals
//!
//! - Not adding a `--wasm-tool` CLI flag (production wiring deferred until
//!   a real CLI consumer exists; tracked as a follow-up).
//! - Not promoting `dasclaw_wasm_tools` to a production dependency of
//!   `dasclaw_cli` (still dev-dep — wasmtime stays out of the shipped
//!   binary).
//! - Not exercising the broader capability matrix (workspace, secrets,
//!   rate limits) — only the http opt-in default-deny row.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::context::JobContext;
use dasclaw_runtime::{Tool, ToolExecutor};
use dasclaw_wasm_tools::{
    Capabilities, NO_HTTP_CAP_WASM, WasmRuntimeConfig, WasmToolRuntime, WasmToolWrapper,
};
use serde_json::json;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;
use fixtures::{ScriptedResponder, text_turn, tool_call_turn};

/// `ToolExecutor` that routes a single named tool call into a
/// `WasmToolWrapper`. Errors from the wrapper (including the
/// `"HTTP capability not granted"` host gate) are mapped into
/// `ToolResult { is_error: true, content: <err.to_string()> }` so the
/// agent loop treats them as reported tool failures rather than host
/// crashes — matching the e13 matrix expectation.
struct WasmToolExecutor {
    tool_name: String,
    wrapper: Arc<WasmToolWrapper>,
}

impl WasmToolExecutor {
    async fn new(tool_name: &str) -> Self {
        // The wasip2 fixture component requests ~17 memory pages
        // (~1.1 MB) at instantiation, above the 1 MB `for_testing` cap.
        // Lift the local limit only; do not touch the global default.
        let mut config = WasmRuntimeConfig::for_testing();
        config.default_limits = config.default_limits.with_memory(4 * 1024 * 1024);
        let runtime = Arc::new(WasmToolRuntime::new(config).expect("init wasm runtime"));
        let prepared = runtime
            .prepare(tool_name, NO_HTTP_CAP_WASM, None)
            .await
            .expect("prepare fixture component");
        let wrapper = WasmToolWrapper::new(Arc::clone(&runtime), prepared, Capabilities::default());
        Self {
            tool_name: tool_name.to_string(),
            wrapper: Arc::new(wrapper),
        }
    }
}

#[async_trait]
impl ToolExecutor for WasmToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        if call.name != self.tool_name {
            return Err(HostError::from(format!(
                "unexpected tool call: {} (test only wires {})",
                call.name, self.tool_name
            )));
        }

        let mut ctx = JobContext::new("a7-e2e", "wasm capability opt-in e2e");
        let outcome = self.wrapper.execute(call.arguments.clone(), &mut ctx).await;

        let (is_error, content) = match outcome {
            Ok(output) => (false, serde_json::to_string(&output).unwrap_or_default()),
            Err(err) => (true, err.to_string()),
        };

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}

fn fixture_tool_def(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: "Sandboxed http fixture (no capability granted)".to_string(),
        parameters: json!({ "type": "object", "properties": {} }),
    }
}

/// e13 main assertion — a WASM tool that calls `host.http-request`
/// without an `http` capability surfaces as `is_error=true` with the
/// host gate message intact, and the agent loop completes normally
/// (the model emits the acknowledgement turn after seeing the error).
#[tokio::test]
async fn req_dasclaw_cli_safety_a7_e13_wasm_no_http_capability_surfaces_as_tool_error() {
    let tool_name = "fixture_http";
    let executor = WasmToolExecutor::new(tool_name).await;

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(tool_name, "call-1", json!({})),
        text_turn("acknowledged: capability gate refused"),
    ]);

    let outcome = run_with_tools_and_hooks(
        responder,
        executor,
        vec![fixture_tool_def(tool_name)],
        HookBundle::noop(),
        "you are running inside a capability-gated CLI",
        "please call the fixture http tool",
    )
    .await;

    let reply = outcome.expect("agent loop must not crash on capability denial");
    assert_eq!(reply, "acknowledged: capability gate refused");
}

/// e13 contract assertion — drive the `ToolExecutor` directly to pin the
/// exact error envelope: `is_error=true` and the host gate string
/// `"HTTP capability not granted"` must travel back to the caller verbatim.
/// This complements the agent-loop test above by isolating the wasm host
/// → tool-result mapping from the broader CLI machinery.
#[tokio::test]
async fn req_dasclaw_cli_safety_a7_e13_wasm_tool_result_carries_host_gate_message() {
    let tool_name = "fixture_http";
    let executor = WasmToolExecutor::new(tool_name).await;

    let call = ToolCall {
        id: "direct-call".to_string(),
        name: tool_name.to_string(),
        arguments: json!({}),
        reasoning: None,
    };

    let result = executor
        .execute(&call)
        .await
        .expect("ToolExecutor must not raise HostError on capability denial");

    assert!(
        result.is_error,
        "capability-gated wasm tool must report is_error=true, got: {result:?}"
    );
    assert!(
        result.content.contains("HTTP capability not granted"),
        "tool result content must carry the host gate string verbatim, got: {}",
        result.content
    );
}
