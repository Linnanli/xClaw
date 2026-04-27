//! GitStaleCheckTool — detect if the current branch is behind its upstream.
//!
//! Uses `git merge-base` and `git rev-list` to determine how many commits
//! the current branch is behind the remote tracking branch. This is useful
//! for prompting the user to rebase before starting work.

use std::time::Instant;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

pub struct GitStaleCheckTool;

impl GitStaleCheckTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitStaleCheckTool {
    fn name(&self) -> &str {
        "git_stale_check"
    }

    fn description(&self) -> &str {
        "Check if the current branch is behind its remote tracking branch. \
         Returns the number of commits behind and ahead, and whether a rebase \
         is recommended."
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

        // Fetch current branch
        let branch_out = run_git(&["branch", "--show-current"], &workdir, None).await?;
        let branch = branch_out.stdout.trim().to_string();

        if branch.is_empty() {
            return Ok(ToolOutput::success(
                serde_json::json!({
                    "detached": true,
                    "message": "HEAD is detached; stale check requires a branch."
                }),
                start.elapsed(),
            ));
        }

        // Find upstream tracking ref
        let upstream_out = run_git(
            &[
                "rev-parse",
                "--abbrev-ref",
                &format!("{}@{{upstream}}", branch),
            ],
            &workdir,
            None,
        )
        .await;

        let upstream = match upstream_out {
            Ok(out) if out.exit_code == 0 => out.stdout.trim().to_string(),
            _ => {
                return Ok(ToolOutput::success(
                    serde_json::json!({
                        "branch": branch,
                        "has_upstream": false,
                        "message": format!("Branch '{}' has no upstream tracking branch.", branch),
                    }),
                    start.elapsed(),
                ));
            }
        };

        // Count commits behind: commits in upstream not in local
        let behind_out = run_git(
            &["rev-list", "--count", &format!("{}..{}", branch, upstream)],
            &workdir,
            None,
        )
        .await?;
        let behind: u64 = behind_out.stdout.trim().parse().unwrap_or(0);

        // Count commits ahead: commits in local not in upstream
        let ahead_out = run_git(
            &["rev-list", "--count", &format!("{}..{}", upstream, branch)],
            &workdir,
            None,
        )
        .await?;
        let ahead: u64 = ahead_out.stdout.trim().parse().unwrap_or(0);

        let needs_rebase = behind > 0;
        let message = if behind == 0 && ahead == 0 {
            format!("Branch '{}' is up to date with '{}'.", branch, upstream)
        } else if behind == 0 {
            format!(
                "Branch '{}' is {} commit(s) ahead of '{}'.",
                branch, ahead, upstream
            )
        } else {
            format!(
                "Branch '{}' is {} commit(s) behind '{}'. Consider rebasing.",
                branch, behind, upstream
            )
        };

        Ok(ToolOutput::success(
            serde_json::json!({
                "branch": branch,
                "upstream": upstream,
                "has_upstream": true,
                "behind": behind,
                "ahead": ahead,
                "needs_rebase": needs_rebase,
                "message": message,
            }),
            start.elapsed(),
        ))
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
    async fn test_stale_check_in_repo() {
        let tool = GitStaleCheckTool::new();
        let ctx = make_ctx();
        let result = tool.execute(serde_json::json!({}), &ctx).await;

        match result {
            Ok(output) => {
                // Should have at least a branch or detached indicator
                let val = &output.result;
                assert!(
                    val.get("branch").is_some() || val.get("detached").is_some(),
                    "should have branch or detached field"
                );
            }
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {
                // git not available in CI — acceptable
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn test_stale_check_bad_path() {
        let tool = GitStaleCheckTool::new();
        let ctx = make_ctx();
        let result = tool
            .execute(
                serde_json::json!({"path": "/nonexistent/path/abc123"}),
                &ctx,
            )
            .await;
        assert!(result.is_err(), "should fail for nonexistent path");
    }
}
