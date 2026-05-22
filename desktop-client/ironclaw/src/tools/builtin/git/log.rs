//! GitLogTool — `git log` with configurable limit and optional file filter.

use std::time::Instant;

use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

const DEFAULT_LIMIT: u64 = 20;
const MAX_LIMIT: u64 = 100;

pub struct GitLogTool;

impl Default for GitLogTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GitLogTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Show commit history. Returns commit hash, author, date, and message. \
         Use `limit` to control how many commits to show."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of commits to show (default: 20, max: 100)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Show only commits that affect this file"
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
        let limit = params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_LIMIT)
            .min(MAX_LIMIT);
        let file_path = params.get("file_path").and_then(|v| v.as_str());

        let workdir = resolve_workdir(path, None)?;
        let limit_str = limit.to_string();

        let mut args = vec![
            "log",
            "--oneline",
            "--no-decorate",
            "--format=%H %ai %an <%ae>%n  %s",
            "-n",
            &limit_str,
        ];

        let dash = "--";
        if let Some(fp) = file_path {
            args.push(dash);
            args.push(fp);
        }

        let output = run_git(&args, &workdir, None).await?;

        if output.stdout.trim().is_empty() {
            return Ok(ToolOutput::text(
                "No commits found.".to_string(),
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
    async fn test_git_log_default() {
        let tool = GitLogTool::new();
        let mut ctx = make_ctx();
        let result = tool.execute(serde_json::json!({}), &mut ctx).await;
        match result {
            Ok(output) => {
                let text = output.result.as_str().unwrap_or_default();
                assert!(!text.is_empty());
            }
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {}
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_git_log_with_limit() {
        let tool = GitLogTool::new();
        let mut ctx = make_ctx();
        let result = tool
            .execute(serde_json::json!({"limit": 3}), &mut ctx)
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
        let tool = GitLogTool::new();
        assert_eq!(tool.risk_level_for(&serde_json::json!({})), RiskLevel::Low);
        assert_eq!(
            tool.requires_approval(&serde_json::json!({})),
            ApprovalRequirement::Never
        );
    }
}
