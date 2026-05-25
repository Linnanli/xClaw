//! Static tool dispatch for [`dasclaw_cli`] (ADR-153 §4.4 step 6).
//!
//! Wires a minimal [`dasclaw_runtime::ToolExecutor`] into the headless
//! CLI so the agent can complete a real tool-using turn. The executor is
//! deliberately self-contained — no MCP, no sandbox, no filesystem
//! mutation — so it stays decoupled from the still-pending `Tool` trait
//! decoupling work in ironclaw (see ADR-153 §4.1).
//!
//! Two builtin demo tools ship with the CLI:
//!
//! - [`builtin_echo`] — returns the `message` argument verbatim.
//! - [`builtin_now`] — returns the current epoch seconds.
//!
//! Higher-level integrations (MCP gateways, dasclaw_fs_tools, ...) are
//! expected to plug their own [`ToolExecutor`] impls in via
//! [`crate::run_with_tools`].

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::ToolExecutor;
use serde_json::{Value, json};

/// Pure-function tool handler: takes the parsed `arguments` JSON object
/// and returns either a textual success payload or a textual error.
///
/// Returning `Err(_)` becomes a [`ToolResult`] with `is_error = true`,
/// which the runtime feeds back to the model as a tool_result block.
pub type ToolHandler = Arc<dyn Fn(&Value) -> Result<String, String> + Send + Sync + 'static>;

/// One named CLI tool: its LLM-facing schema plus its local handler.
#[derive(Clone)]
pub struct CliTool {
    /// LLM-facing definition (name + description + JSON schema). Sent
    /// verbatim to the provider via [`crate::run_with_tools`].
    pub definition: ToolDefinition,
    /// Local handler invoked when the model calls the tool.
    pub handler: ToolHandler,
}

impl CliTool {
    /// Build a [`CliTool`] from its parts.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: impl Fn(&Value) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            definition: ToolDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
            handler: Arc::new(handler),
        }
    }
}

/// In-memory [`ToolExecutor`] that dispatches by tool name.
///
/// Mirrors the design of `claw-code`'s `StaticToolExecutor` but uses an
/// async-trait impl so it slots into [`dasclaw_runtime::AgentBuilder`]
/// directly.
#[derive(Default, Clone)]
pub struct StaticToolExecutor {
    tools: BTreeMap<String, CliTool>,
}

impl StaticToolExecutor {
    /// Build an empty executor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Later registrations with the same name replace
    /// earlier ones — same semantics as `BTreeMap::insert`.
    #[must_use]
    pub fn with_tool(mut self, tool: CliTool) -> Self {
        self.tools.insert(tool.definition.name.clone(), tool);
        self
    }

    /// All tool definitions, in deterministic name order. Pass this to
    /// [`crate::run_with_tools`] so the model sees the same set the
    /// executor can serve.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition.clone()).collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the executor has no registered tools.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[async_trait]
impl ToolExecutor for StaticToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        // Unknown tool → feed the failure back to the model rather than
        // bubbling a HostError. The runtime treats `is_error = true` as a
        // tool_result error the LLM can recover from.
        let Some(tool) = self.tools.get(&call.name) else {
            return Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: format!("unknown tool: {}", call.name),
                is_error: true,
            });
        };
        let (content, is_error) = match (tool.handler)(&call.arguments) {
            Ok(c) => (c, false),
            Err(e) => (e, true),
        };
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}

/// Builtin: `echo` — returns the `message` argument verbatim.
///
/// JSON schema: `{ "message": string }`. Missing or non-string `message`
/// returns an error to the model.
pub fn builtin_echo() -> CliTool {
    CliTool::new(
        "echo",
        "Return the `message` argument back to the caller verbatim. \
         Useful for smoke-testing the agent's tool-use loop.",
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The text to echo back."
                }
            },
            "required": ["message"]
        }),
        |args| {
            args.get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "echo: missing required string argument `message`".to_string())
        },
    )
}

/// Builtin: `now` — returns the current Unix epoch seconds as a string.
///
/// No arguments. Useful for sanity-checking that the agent can drive a
/// tool with side effects (clock read).
pub fn builtin_now() -> CliTool {
    CliTool::new(
        "now",
        "Return the current Unix epoch time in seconds as a decimal \
         integer string. Takes no arguments.",
        json!({
            "type": "object",
            "properties": {}
        }),
        |_args| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .map_err(|e| format!("now: system clock is before UNIX_EPOCH: {e}"))
        },
    )
}

/// Returns a [`StaticToolExecutor`] preloaded with the two builtin demo
/// tools ([`builtin_echo`], [`builtin_now`]).
///
/// Used by `dasclaw-cli run --enable-tools` and by integration tests.
pub fn default_builtins() -> StaticToolExecutor {
    StaticToolExecutor::new()
        .with_tool(builtin_echo())
        .with_tool(builtin_now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            arguments: args,
            reasoning: None,
        }
    }

    #[tokio::test]
    async fn req_dasclaw_cli_tools_a1_unknown_tool_returns_error_result() {
        let executor = default_builtins();
        let result = executor
            .execute(&call("does-not-exist", json!({})))
            .await
            .expect("executor returned HostError unexpectedly");
        assert!(result.is_error, "unknown tool must flag is_error=true");
        assert!(
            result.content.contains("unknown tool"),
            "expected diagnostic message, got: {}",
            result.content
        );
        assert_eq!(result.tool_call_id, "call-1");
    }

    #[tokio::test]
    async fn req_dasclaw_cli_tools_a2_echo_returns_message_verbatim() {
        let executor = default_builtins();
        let result = executor
            .execute(&call("echo", json!({ "message": "hello" })))
            .await
            .expect("executor returned HostError unexpectedly");
        assert!(!result.is_error);
        assert_eq!(result.content, "hello");
        assert_eq!(result.name, "echo");
    }

    #[tokio::test]
    async fn req_dasclaw_cli_tools_a3_echo_missing_argument_is_tool_error() {
        let executor = default_builtins();
        let result = executor
            .execute(&call("echo", json!({})))
            .await
            .expect("executor returned HostError unexpectedly");
        assert!(result.is_error);
        assert!(result.content.contains("missing required"));
    }

    #[tokio::test]
    async fn req_dasclaw_cli_tools_a4_now_returns_decimal_seconds() {
        let executor = default_builtins();
        let result = executor
            .execute(&call("now", json!({})))
            .await
            .expect("executor returned HostError unexpectedly");
        assert!(!result.is_error);
        // Parse as u64 to confirm it is a decimal integer string.
        let secs: u64 = result
            .content
            .parse()
            .expect("`now` should return decimal seconds");
        assert!(secs > 1_700_000_000, "epoch seconds should be post-2023");
    }

    #[test]
    fn definitions_are_sorted_and_complete() {
        let executor = default_builtins();
        let names: Vec<_> = executor.definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["echo".to_string(), "now".to_string()]);
    }
}
