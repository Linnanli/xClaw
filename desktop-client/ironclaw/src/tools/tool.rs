//! `Tool` trait re-export and ironclaw-side trait conformance tests.
//!
//! Per [ADR-154 §3.1 amendment](../../../../../docs/plans/architecture-refactor/adr-154-jobcontext-core-trait-split.md),
//! the `Tool` trait was physically moved to `dasclaw_runtime` (not
//! `dasclaw_tool`) because the workspace dep direction is
//! `dasclaw_runtime → dasclaw_tool`. ironclaw keeps a thin re-export so
//! every existing path resolves unchanged.
//!
//! Pure-data primitives (`ApprovalRequirement`, `ApprovalContext`,
//! `RiskLevel`, `ToolDomain`, `ToolOutput`, `ToolSchema`,
//! `ToolDiscoverySummary`, `ToolRateLimitConfig`, `WebhookCapability`) and
//! parameter helpers (`require_str`, `require_param`, `redact_params`,
//! `validate_tool_schema`) continue to live in the shared `dasclaw_tool`
//! crate.

pub use dasclaw_runtime::Tool;

pub use dasclaw_tool::{
    ApprovalContext, ApprovalRequirement, RiskLevel, ToolDiscoverySummary, ToolDomain, ToolError,
    ToolOutput, ToolRateLimitConfig, ToolSchema, redact_params, require_param, require_str,
    validate_tool_schema,
};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;

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
