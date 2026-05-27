//! Generic [`Tool`] → [`ToolExecutor`] adapter.
//!
//! Bridges the application-layer [`Tool`] trait (which takes structured
//! `serde_json::Value` parameters plus a mutable
//! [`JobContextCore`](crate::JobContextCore) and returns a typed
//! [`ToolOutput`](dasclaw_tool::ToolOutput)) into the
//! [`ToolExecutor`] seam consumed by [`crate::Agent`] (which takes a
//! `ToolCall` and returns a `ToolResult`).
//!
//! ## Why this lives here
//!
//! Before this adapter, every tool family (MCP gateway in
//! `dasclaw_mcp::executor`, the static builtin runner in
//! `dasclaw_cli::tools::StaticToolExecutor`, sub-agents, …) hand-rolled
//! their own `impl ToolExecutor`. That meant any host that wanted to
//! drive a single `Tool` implementation through `Agent` (the headless
//! `dasclaw-cli` shell-tool wiring is the motivating case) had to
//! duplicate the parameter / output / error mapping yet again. This
//! adapter is the missing piece of the runtime so that
//! `Arc::new(ToolToExecutorAdapter::new(MyTool::default()))` is enough
//! to plug a tool into an agent loop.
//!
//! ## Design
//!
//! * The adapter holds an `Arc<T>` to its underlying tool, so cloning
//!   the executor (e.g. wrapping it in [`crate::CompositeToolExecutor`])
//!   is cheap and the same tool instance is reused across invocations.
//! * It owns a shared `Arc<Mutex<JobContext>>` because `Tool::execute`
//!   takes `&mut dyn JobContextCore`. State written into the context by
//!   one invocation (e.g. `set_metadata`, `tool_output_stash` mutations)
//!   is visible to the next, matching the contract of the in-process
//!   `JobContext` used by ironclaw.
//! * Parameter wire format: [`dasclaw_core::messages::ToolCall::arguments`]
//!   is already a parsed `serde_json::Value`. The adapter forwards it
//!   directly into [`Tool::execute`] without re-parsing.
//! * Error mapping mirrors [`crate::composite_executor`] and the MCP
//!   executor: a tool-level failure ([`ToolError`]) becomes
//!   `ToolResult { is_error: true, content: <Display> }` rather than a
//!   `HostError`. The model gets the failure back as a recoverable
//!   `tool_result` block; the agent loop continues. Only infrastructural
//!   failures (poisoned mutex, …) would bubble up as `HostError`, and
//!   the current implementation has no path that can reach one.
//!
//! ## Approval / risk metadata
//!
//! `Tool::requires_approval` and `Tool::risk_level_for` are inspection
//! hooks for callers (e.g. the hook chain) — they do **not** gate the
//! tool's `execute` itself. The adapter therefore does not consult
//! them: approval enforcement lives in
//! [`dasclaw_core::hooks::BeforeToolCall`], not inside the executor
//! seam.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use dasclaw_core::messages::{ToolCall, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_tool::ToolOutput;

use crate::agent::ToolExecutor;
use crate::context::state::JobContext;
use crate::tool::Tool;

/// Generic [`Tool`] → [`ToolExecutor`] adapter.
///
/// See the [module documentation](self) for design rationale.
pub struct ToolToExecutorAdapter<T: Tool + 'static> {
    tool: Arc<T>,
    ctx: Arc<Mutex<JobContext>>,
}

impl<T: Tool + 'static> ToolToExecutorAdapter<T> {
    /// Wrap `tool` with a fresh default [`JobContext`].
    ///
    /// Suitable for headless callers (`dasclaw-cli`, smoke tests) that
    /// don't have a host-supplied job context. The default context has
    /// no background tasks, no exclusive resources, and zero-cost
    /// `Arc`-based fields — see
    /// [`JobContext::default`](crate::context::state::JobContext::default).
    pub fn new(tool: T) -> Self {
        Self::with_context(tool, JobContext::default())
    }

    /// Wrap `tool` with a caller-supplied [`JobContext`].
    ///
    /// Use this when the host wants to propagate `extra_env`,
    /// `workspace_root` metadata, feature flags, or a recording HTTP
    /// interceptor into the tool's environment.
    pub fn with_context(tool: T, ctx: JobContext) -> Self {
        Self {
            tool: Arc::new(tool),
            ctx: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Read-only access to the underlying tool. Useful for tests.
    pub fn tool(&self) -> &T {
        &self.tool
    }

    /// Clone the shared context handle. State written by tool
    /// invocations on this adapter is visible through this handle.
    pub fn context(&self) -> Arc<Mutex<JobContext>> {
        self.ctx.clone()
    }
}

#[async_trait]
impl<T: Tool + Send + Sync + 'static> ToolExecutor for ToolToExecutorAdapter<T> {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let mut ctx = self.ctx.lock().await;
        let outcome = self.tool.execute(call.arguments.clone(), &mut *ctx).await;
        drop(ctx);

        let (content, is_error) = match outcome {
            Ok(output) => (render_tool_output(&output), false),
            Err(err) => (err.to_string(), true),
        };

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}

/// Render a [`ToolOutput`] into the [`ToolResult::content`] string the
/// model will see in its next `tool_result` block.
///
/// Mirrors the MCP gateway's `map_call_result` and the static executor
/// in `dasclaw_cli::tools`: plain text comes through verbatim, anything
/// structured is serialised as compact JSON so the LLM can still parse
/// it back. We deliberately do **not** wrap text outputs in quotes
/// (which `Value::to_string()` would do), to keep the wire format
/// stable across `ToolOutput::text` and `ToolOutput::success`.
fn render_tool_output(output: &ToolOutput) -> String {
    match &output.result {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use dasclaw_tool::{ToolError, ToolOutput};

    use crate::job_context::JobContextCore;

    /// Tool that echoes back a `text` argument as a `ToolOutput::text`.
    struct TextEchoTool;

    #[async_trait]
    impl Tool for TextEchoTool {
        fn name(&self) -> &str {
            "text_echo"
        }
        fn description(&self) -> &str {
            "echo"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]})
        }
        async fn execute(
            &self,
            params: Value,
            _ctx: &mut dyn JobContextCore,
        ) -> Result<ToolOutput, ToolError> {
            let text = params
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::ExecutionFailed("missing text".into()))?;
            Ok(ToolOutput::text(text.to_owned(), Duration::from_millis(0)))
        }
    }

    /// Tool that returns a structured `ToolOutput::success` payload.
    struct StructuredTool;

    #[async_trait]
    impl Tool for StructuredTool {
        fn name(&self) -> &str {
            "structured"
        }
        fn description(&self) -> &str {
            "structured"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type":"object","properties":{},"required":[]})
        }
        async fn execute(
            &self,
            _params: Value,
            _ctx: &mut dyn JobContextCore,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(
                json!({"exit_code": 1, "output": "Operation not permitted", "sandboxed": true}),
                Duration::from_millis(0),
            ))
        }
    }

    /// Tool that always fails with `ToolError::NotAuthorized`.
    struct AlwaysDenyTool;

    #[async_trait]
    impl Tool for AlwaysDenyTool {
        fn name(&self) -> &str {
            "deny"
        }
        fn description(&self) -> &str {
            "deny"
        }
        fn parameters_schema(&self) -> Value {
            json!({"type":"object","properties":{},"required":[]})
        }
        async fn execute(
            &self,
            _params: Value,
            _ctx: &mut dyn JobContextCore,
        ) -> Result<ToolOutput, ToolError> {
            Err(ToolError::NotAuthorized("sandbox: blocked write".into()))
        }
    }

    fn make_call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: format!("call_{name}"),
            name: name.to_owned(),
            arguments: args,
            reasoning: None,
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_adapter_text_success_passes_text_through_unquoted() {
        let adapter = ToolToExecutorAdapter::new(TextEchoTool);
        let result = adapter
            .execute(&make_call("text_echo", json!({"text": "hello"})))
            .await
            .expect("infra ok");
        assert!(!result.is_error);
        assert_eq!(result.content, "hello");
        assert_eq!(result.name, "text_echo");
        assert_eq!(result.tool_call_id, "call_text_echo");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_adapter_structured_success_serialises_as_json() {
        let adapter = ToolToExecutorAdapter::new(StructuredTool);
        let result = adapter
            .execute(&make_call("structured", json!({})))
            .await
            .expect("infra ok");
        assert!(!result.is_error);
        // Compact JSON so the LLM can parse it back, and so callers can
        // grep for keywords like `Operation not permitted` / `sandboxed`.
        assert!(
            result.content.contains("Operation not permitted"),
            "content was: {}",
            result.content
        );
        assert!(result.content.contains("\"sandboxed\":true"));
        assert!(result.content.contains("\"exit_code\":1"));
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_adapter_tool_error_becomes_is_error_true() {
        let adapter = ToolToExecutorAdapter::new(AlwaysDenyTool);
        let result = adapter
            .execute(&make_call("deny", json!({})))
            .await
            .expect("infra ok");
        assert!(result.is_error);
        // `ToolError::NotAuthorized` Display includes the inner message;
        // downstream stderr / stdout scrapers can search for "sandbox".
        assert!(
            result.content.to_lowercase().contains("sandbox"),
            "content was: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_adapter_context_is_shared_across_invocations() {
        let adapter = ToolToExecutorAdapter::new(TextEchoTool);

        // Mutate the context out-of-band; the next execute() must see it.
        let ctx_handle = adapter.context();
        {
            let mut ctx = ctx_handle.lock().await;
            ctx.set_metadata(json!({"workspace_root": "/tmp/test"}));
        }

        let result = adapter
            .execute(&make_call("text_echo", json!({"text": "ok"})))
            .await
            .expect("infra ok");
        assert!(!result.is_error);

        let ctx = ctx_handle.lock().await;
        assert_eq!(
            ctx.metadata().get("workspace_root").and_then(Value::as_str),
            Some("/tmp/test"),
        );
    }
}
