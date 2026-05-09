//! GitPushTool — push commits to a remote.
//!
//! High risk: publishes local commits to a remote. **Always** requires explicit approval.

use std::time::Instant;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

pub struct GitPushTool;

impl Default for GitPushTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GitPushTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitPushTool {
    fn name(&self) -> &str {
        "git_push"
    }

    fn description(&self) -> &str {
        "Push commits to a remote repository. \
         Defaults to 'origin' and the current branch. \
         Use `force: true` for --force-with-lease (safer than --force)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "remote": {
                    "type": "string",
                    "description": "Remote name (default: origin)"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch to push (default: current branch)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Use --force-with-lease (default: false)"
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

        let remote = params
            .get("remote")
            .and_then(|v| v.as_str())
            .unwrap_or("origin");
        let branch = params.get("branch").and_then(|v| v.as_str());
        let force = params
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut args = vec!["push"];
        if force {
            args.push("--force-with-lease");
        }
        args.push(remote);
        if let Some(b) = branch {
            args.push(b);
        }

        let output = run_git(&args, &workdir, None).await?;
        Ok(ToolOutput::text(output.stdout, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn risk_level_for(&self, _params: &serde_json::Value) -> RiskLevel {
        // Push is always High risk regardless of --force; we keep both paths
        // explicit at the schema level via `requires_approval = Always`.
        RiskLevel::High
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Always
    }

    fn requires_sanitization(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_high_risk() {
        let tool = GitPushTool::new();
        assert_eq!(tool.risk_level_for(&serde_json::json!({})), RiskLevel::High);
        assert_eq!(
            tool.risk_level_for(&serde_json::json!({"force": true})),
            RiskLevel::High
        );
    }

    #[test]
    fn test_always_requires_approval() {
        let tool = GitPushTool::new();
        assert_eq!(
            tool.requires_approval(&serde_json::json!({})),
            ApprovalRequirement::Always
        );
        assert_eq!(
            tool.requires_approval(&serde_json::json!({"force": true})),
            ApprovalRequirement::Always
        );
    }
}
