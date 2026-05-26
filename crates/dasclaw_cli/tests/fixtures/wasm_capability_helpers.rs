//! Shared helpers for the broader wasm capability-matrix e2e tests
//! (ADR-153 §1.1 A7 follow-up, issue #869).
//!
//! Builds a `ToolExecutor` that routes a single named tool call into a
//! `WasmToolWrapper` configured with caller-supplied `Capabilities` and an
//! optional `HttpInterceptor`. Errors from the wrapper surface as
//! `ToolResult { is_error: true, content: <err.to_string()> }` so the agent
//! loop treats capability denials as reported tool failures, matching the
//! contract pinned by the A7 baseline test.
//!
//! Kept separate from `safety_wasm_capability_optin_e2e.rs` so the A7
//! baseline (http opt-in) stays self-contained, while the workspace /
//! secrets / rate-limit follow-up tests share one builder.

#![allow(dead_code)] // each test file uses a different subset of helpers

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::context::JobContext;
use dasclaw_runtime::recording::HttpInterceptor;
use dasclaw_runtime::{Tool, ToolExecutor};
use dasclaw_wasm_tools::{
    Capabilities, NO_HTTP_CAP_WASM, WasmRuntimeConfig, WasmToolRuntime, WasmToolWrapper,
};
use serde_json::json;

/// `ToolExecutor` that routes one named tool call through a single
/// `WasmToolWrapper`. Mirrors the helper in
/// `safety_wasm_capability_optin_e2e.rs` but is parameterised by
/// `Capabilities` + optional `HttpInterceptor` so the capability-matrix
/// follow-up tests can share construction logic.
pub struct CapWasmExecutor {
    tool_name: String,
    wrapper: Arc<WasmToolWrapper>,
}

impl CapWasmExecutor {
    pub async fn build(
        tool_name: &str,
        capabilities: Capabilities,
        interceptor: Option<Arc<dyn HttpInterceptor>>,
    ) -> Self {
        // Match the A7 baseline: lift the per-instance memory cap to
        // 4 MiB so the wasip2 fixture's ~17 starting pages fit (the
        // `for_testing` default is 1 MiB).
        let mut config = WasmRuntimeConfig::for_testing();
        config.default_limits = config.default_limits.with_memory(4 * 1024 * 1024);
        let runtime = Arc::new(WasmToolRuntime::new(config).expect("init wasm runtime"));
        let prepared = runtime
            .prepare(tool_name, NO_HTTP_CAP_WASM, None)
            .await
            .expect("prepare fixture component");

        let mut wrapper = WasmToolWrapper::new(Arc::clone(&runtime), prepared, capabilities);
        if let Some(interceptor) = interceptor {
            wrapper = wrapper.with_http_interceptor(interceptor);
        }

        Self {
            tool_name: tool_name.to_string(),
            wrapper: Arc::new(wrapper),
        }
    }
}

#[async_trait]
impl ToolExecutor for CapWasmExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        if call.name != self.tool_name {
            return Err(HostError::from(format!(
                "unexpected tool call: {} (test only wires {})",
                call.name, self.tool_name
            )));
        }

        let mut ctx = JobContext::new("a7-followup", "wasm capability matrix e2e");
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

/// Build the `ToolDefinition` advertised to the scripted LLM.
pub fn fixture_tool_def(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: "Sandboxed capability-matrix fixture (workspace / secrets / rate_limit)"
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["http", "workspace", "secrets", "rate_limit"]
                }
            }
        }),
    }
}
