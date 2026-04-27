//! GitStatusTool — `git status` with porcelain output.

use std::time::Instant;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

pub struct GitStatusTool;

impl GitStatusTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Show the working tree status. Returns branch name, staged changes, \
         unstaged changes, and untracked files."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
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

        // Branch info
        let branch = run_git(&["branch", "--show-current"], &workdir, None).await?;

        // Porcelain status (machine-parseable)
        let status = run_git(&["status", "--porcelain=v1", "--branch"], &workdir, None).await?;

        let result = serde_json::json!({
            "branch": branch.stdout.trim(),
            "status": status.stdout,
            "exit_code": status.exit_code,
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn risk_level_for(&self, _params: &serde_json::Value) -> RiskLevel {
        RiskLevel::Low
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    fn requires_sanitization(&self) -> bool {
        false
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
    async fn test_git_status_in_repo() {
        let tool = GitStatusTool::new();
        let ctx = make_ctx();
        let result = tool.execute(serde_json::json!({}), &ctx).await;

        match result {
            Ok(output) => {
                // Result is JSON: { branch, status, exit_code }
                assert!(
                    output.result.get("branch").is_some(),
                    "should have branch field"
                );
            }
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {}
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_git_status_bad_path() {
        let tool = GitStatusTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(serde_json::json!({"path": "/nonexistent/path"}), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_level() {
        let tool = GitStatusTool::new();
        assert_eq!(tool.risk_level_for(&serde_json::json!({})), RiskLevel::Low);
    }

    #[test]
    fn test_approval_never() {
        let tool = GitStatusTool::new();
        assert_eq!(
            tool.requires_approval(&serde_json::json!({})),
            ApprovalRequirement::Never
        );
    }
}
