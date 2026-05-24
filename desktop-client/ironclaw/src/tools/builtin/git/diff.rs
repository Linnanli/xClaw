//! GitDiffTool — `git diff` with optional staging/file scope.

use std::time::Instant;

use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

pub struct GitDiffTool;

impl Default for GitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GitDiffTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Show changes between commits, the working tree, and/or the staging area. \
         Use `staged: true` to see staged changes (like `git diff --cached`)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "If true, show staged changes (--cached). Default: false"
                },
                "file_path": {
                    "type": "string",
                    "description": "Limit diff to a specific file"
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
        _ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let start = Instant::now();
        let path = params.get("path").and_then(|v| v.as_str());
        let staged = params
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let file_path = params.get("file_path").and_then(|v| v.as_str());
        let workdir = resolve_workdir(path, None)?;

        let mut args: Vec<&str> = vec!["diff", "--stat", "--patch"];
        if staged {
            args.push("--cached");
        }

        // We need to own the "--" + file_path string if present
        let dash = "--";
        if let Some(fp) = file_path {
            args.push(dash);
            args.push(fp);
        }

        let output = run_git(&args, &workdir, None).await?;

        if output.stdout.trim().is_empty() {
            let label = if staged { "staged" } else { "unstaged" };
            return Ok(ToolOutput::text(
                format!("No {} changes.", label),
                start.elapsed(),
            ));
        }

        Ok(ToolOutput::text(output.stdout, start.elapsed()))
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
    use dasclaw_runtime::context::JobContext;

    fn make_ctx() -> JobContext {
        JobContext::default()
    }

    #[tokio::test]
    async fn test_git_diff_no_changes() {
        let tool = GitDiffTool::new();
        let mut ctx = make_ctx();
        // In a clean working tree, should return "No unstaged changes."
        let result = tool.execute(serde_json::json!({}), &mut ctx).await;
        match result {
            Ok(output) => {
                let text = output.result.as_str().unwrap_or_default();
                // Either shows diff or "No unstaged changes."
                assert!(!text.is_empty());
            }
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {}
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_git_diff_staged() {
        let tool = GitDiffTool::new();
        let mut ctx = make_ctx();
        let result = tool
            .execute(serde_json::json!({"staged": true}), &mut ctx)
            .await;
        match result {
            Ok(output) => {
                let text = output.result.as_str().unwrap_or_default();
                assert!(!text.is_empty());
            }
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {}
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[test]
    fn test_risk_and_approval() {
        let tool = GitDiffTool::new();
        assert_eq!(tool.risk_level_for(&serde_json::json!({})), RiskLevel::Low);
        assert_eq!(
            tool.requires_approval(&serde_json::json!({})),
            ApprovalRequirement::Never
        );
    }
}
