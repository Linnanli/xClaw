//! GitCommitTool — stage and commit changes.
//!
//! Supports `--all` (auto-stage tracked files) and a required commit message.
//! Medium risk: modifies repository history. Requires approval unless auto-approved.

use std::time::Instant;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

const MAX_MESSAGE_LEN: usize = 1000;

pub struct GitCommitTool;

impl Default for GitCommitTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GitCommitTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Create a git commit with the given message. \
         Use `all: true` to auto-stage all tracked file changes before committing."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["message"],
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message (max 1000 chars)"
                },
                "all": {
                    "type": "boolean",
                    "description": "If true, auto-stage all tracked file changes (git commit -a)"
                },
                "path": {
                    "type": "string",
                    "description": "Working directory (default: current directory)"
                }
            }
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let start = Instant::now();
        let path = params.get("path").and_then(|v| v.as_str());
        let workdir = resolve_workdir(path, None)?;

        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParameters("Missing required 'message' parameter".into())
            })?;

        if message.trim().is_empty() {
            return Err(ToolError::InvalidParameters(
                "Commit message cannot be empty".into(),
            ));
        }

        let truncated_msg = if message.len() > MAX_MESSAGE_LEN {
            &message[..MAX_MESSAGE_LEN]
        } else {
            message
        };

        let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut args = vec!["commit"];
        if all {
            args.push("-a");
        }
        args.push("-m");
        args.push(truncated_msg);

        let output = run_git(&args, &workdir, None).await?;
        Ok(ToolOutput::text(output.stdout, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn risk_level_for(&self, _params: &serde_json::Value) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::UnlessAutoApproved
    }

    fn requires_sanitization(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::JobContext;

    fn make_ctx() -> JobContext {
        JobContext::default()
    }

    #[tokio::test]
    async fn test_missing_message() {
        let tool = GitCommitTool::new();
        let ctx = make_ctx();
        let result = tool.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
        if let Err(ToolError::InvalidParameters(msg)) = result {
            assert!(msg.contains("message"));
        }
    }

    #[tokio::test]
    async fn test_empty_message() {
        let tool = GitCommitTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(serde_json::json!({"message": "  "}), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_and_approval() {
        let tool = GitCommitTool::new();
        assert_eq!(
            tool.risk_level_for(&serde_json::json!({})),
            RiskLevel::Medium
        );
        assert_eq!(
            tool.requires_approval(&serde_json::json!({})),
            ApprovalRequirement::UnlessAutoApproved
        );
    }

    #[test]
    fn test_requires_sanitization() {
        let tool = GitCommitTool::new();
        assert!(tool.requires_sanitization());
    }
}
