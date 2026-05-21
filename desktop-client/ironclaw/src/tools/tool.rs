//! Tool trait and helpers.
//!
//! Pure-data primitives (`ApprovalRequirement`, `ApprovalContext`,
//! `RiskLevel`, `ToolDomain`, `ToolOutput`, `ToolSchema`,
//! `ToolDiscoverySummary`, `ToolRateLimitConfig`) and parameter helpers
//! (`require_str`, `require_param`, `redact_params`, `validate_tool_schema`)
//! moved to the shared [`dasclaw_tool`] crate so any dasclaw host
//! (desktop, CLI, headless, admin backend) can describe and dispatch
//! tools without depending on this ironclaw crate.
//!
//! The [`Tool`] trait itself still lives here because it depends on
//! [`crate::context::JobContext`] and
//! [`crate::tools::wasm::WebhookCapability`]; decoupling those is
//! tracked as a follow-up issue.

use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;

pub use dasclaw_tool::{
    ApprovalContext, ApprovalRequirement, RiskLevel, ToolDiscoverySummary, ToolDomain, ToolError,
    ToolOutput, ToolRateLimitConfig, ToolSchema, redact_params, require_param, require_str,
    validate_tool_schema,
};

/// Trait for tools that the agent can use.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get a description of what the tool does.
    fn description(&self) -> &str;

    /// Get the JSON Schema for the tool's parameters.
    fn parameters_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given parameters.
    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError>;

    /// Estimate the cost of running this tool with the given parameters.
    fn estimated_cost(&self, _params: &serde_json::Value) -> Option<Decimal> {
        None
    }

    /// Estimate how long this tool will take with the given parameters.
    fn estimated_duration(&self, _params: &serde_json::Value) -> Option<Duration> {
        None
    }

    /// Whether this tool's output needs sanitization.
    ///
    /// Returns true for tools that interact with external services,
    /// where the output might contain malicious content.
    fn requires_sanitization(&self) -> bool {
        true
    }

    /// Risk level for a specific invocation of this tool.
    ///
    /// Defaults to `Low` (read-only, safe). Override for tools whose risk
    /// depends on the parameters — the shell tool classifies commands into
    /// `Low` / `Medium` / `High` based on the command string.
    ///
    /// The worker logs this value with every tool call so operators can audit
    /// the risk level at which each execution was classified.
    fn risk_level_for(&self, _params: &serde_json::Value) -> RiskLevel {
        RiskLevel::Low
    }

    /// Whether this tool invocation requires user approval.
    ///
    /// Returns `Never` by default (most tools run in a sandboxed environment).
    /// Override to return `UnlessAutoApproved` for tools that need approval
    /// but can be session-auto-approved, or `Always` for invocations that
    /// must always prompt (e.g. destructive shell commands, HTTP with auth).
    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    /// Maximum time this tool is allowed to run before the caller kills it.
    /// Override for long-running tools like sandbox execution.
    /// Default: 60 seconds.
    fn execution_timeout(&self) -> Duration {
        Duration::from_secs(60)
    }

    /// Where this tool should execute.
    ///
    /// `Orchestrator` tools run in the main agent process (safe, no FS access).
    /// `Container` tools run inside Docker containers (shell, file ops).
    ///
    /// Default: `Orchestrator` (safe for the main process).
    fn domain(&self) -> ToolDomain {
        ToolDomain::Orchestrator
    }

    /// Parameter names whose values must be redacted before logging, hooks, and approvals.
    ///
    /// The agent framework replaces these parameter values with `"[REDACTED]"` before:
    /// - Writing to debug logs
    /// - Storing in `ActionRecord` (in-memory job history)
    /// - Recording in `TurnToolCall` (session state)
    /// - Sending to `BeforeToolCall` hooks
    /// - Displaying in the approval UI
    ///
    /// **The `execute()` method still receives the original, unredacted parameters.**
    /// Redaction only applies to the observability and audit paths, not execution.
    ///
    /// Use this for tools that accept plaintext secrets as parameters (e.g. `secret_save`).
    fn sensitive_params(&self) -> &[&str] {
        &[]
    }

    /// Per-invocation rate limit for this tool.
    ///
    /// Return `Some(config)` to throttle how often this tool can be called per user.
    /// Read-only tools (echo, time, json, file_read, memory_search, etc.) should
    /// return `None`. Write/external tools (shell, http, file_write, memory_write,
    /// create_job) should return sensible limits to prevent runaway agents.
    ///
    /// Rate limits are per-user, per-tool, and in-memory (reset on restart).
    /// This is orthogonal to `requires_approval()` — a tool can be both
    /// approval-gated and rate limited. Rate limit is checked first (cheaper).
    ///
    /// Default: `None` (no rate limiting).
    fn rate_limit_config(&self) -> Option<ToolRateLimitConfig> {
        None
    }

    /// Optional host-side webhook verification configuration for this tool.
    ///
    /// When present, `/webhook/tools/{tool}` validates shared secret/signatures
    /// before invoking the tool. Tools should then only handle payload normalization.
    fn webhook_capability(&self) -> Option<crate::tools::wasm::WebhookCapability> {
        None
    }

    /// Full parameter schema for discovery and coercion purposes.
    ///
    /// Unlike `parameters_schema()` (which may be permissive to keep the tools
    /// array compact), this returns the complete typed schema. Used by the
    /// `tool_info` built-in and by WASM parameter coercion.
    ///
    /// Default: delegates to `parameters_schema()`.
    fn discovery_schema(&self) -> serde_json::Value {
        self.parameters_schema()
    }

    /// Curated discovery guidance used by `tool_info(detail: "summary")`.
    ///
    /// Default: no custom summary; callers may derive a minimal fallback from
    /// `discovery_schema()`.
    fn discovery_summary(&self) -> Option<ToolDiscoverySummary> {
        None
    }

    /// Get the tool schema for LLM function calling.
    fn schema(&self) -> ToolSchema {
        let parameters = self.parameters_schema();
        let has_discovery_hint =
            self.discovery_summary().is_some() || self.discovery_schema() != parameters;
        let description = if has_discovery_hint {
            format!(
                "{} (call tool_info(name: \"{}\", detail: \"summary\") for rules/examples or detail: \"schema\" for the full discovery schema)",
                self.description(),
                self.name()
            )
        } else {
            self.description().to_string()
        };
        ToolSchema {
            name: self.name().to_string(),
            description,
            parameters,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::JobContext;

    /// A simple no-op tool for testing.
    #[derive(Debug)]
    pub struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn description(&self) -> &str {
            "Echoes back the input message. Useful for testing."
        }

        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The message to echo back"
                    }
                },
                "required": ["message"]
            })
        }

        async fn execute(
            &self,
            params: serde_json::Value,
            _ctx: &mut dyn dasclaw_runtime::JobContextCore,
        ) -> Result<ToolOutput, ToolError> {
            let message = require_str(&params, "message")?;

            Ok(ToolOutput::text(message, Duration::from_millis(1)))
        }

        fn requires_sanitization(&self) -> bool {
            false // Echo is a trusted internal tool
        }
    }

    #[tokio::test]
    async fn test_echo_tool() {
        let tool = EchoTool;
        let mut ctx = JobContext::default();

        let result = tool
            .execute(serde_json::json!({"message": "hello"}), &mut ctx)
            .await
            .unwrap();

        assert_eq!(result.result, serde_json::json!("hello"));
    }

    #[test]
    fn test_tool_schema() {
        let tool = EchoTool;
        let schema = tool.schema();

        assert_eq!(schema.name, "echo");
        assert!(!schema.description.is_empty());
    }

    #[test]
    fn test_execution_timeout_default() {
        let tool = EchoTool;
        assert_eq!(tool.execution_timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_requires_approval_default() {
        let tool = EchoTool;
        // Default requires_approval() returns Never.
        assert_eq!(
            tool.requires_approval(&serde_json::json!({"message": "hi"})),
            ApprovalRequirement::Never
        );
        assert!(!ApprovalRequirement::Never.is_required());
        assert!(ApprovalRequirement::UnlessAutoApproved.is_required());
        assert!(ApprovalRequirement::Always.is_required());
    }
}
