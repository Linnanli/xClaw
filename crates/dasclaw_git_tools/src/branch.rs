//! GitBranchTool — list, create, switch, and delete branches.

use std::time::Instant;

use dasclaw_runtime::Tool;
use dasclaw_tool::{ApprovalRequirement, RiskLevel, ToolDomain, ToolError, ToolOutput};

use super::runner::{resolve_workdir, run_git};

pub struct GitBranchTool;

impl Default for GitBranchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GitBranchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for GitBranchTool {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn description(&self) -> &str {
        "Manage git branches. Actions: list (default), create, switch, delete."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "switch", "delete"],
                    "description": "Branch action (default: list)"
                },
                "name": {
                    "type": "string",
                    "description": "Branch name (required for create/switch/delete)"
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
        let workdir = resolve_workdir(path, None)?;

        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");

        let name = params.get("name").and_then(|v| v.as_str());

        let output = match action {
            "list" => run_git(&["branch", "-a", "--no-color"], &workdir, None).await?,
            "create" => {
                let branch = require_name(name)?;
                run_git(&["branch", branch], &workdir, None).await?
            }
            "switch" => {
                let branch = require_name(name)?;
                run_git(&["switch", branch], &workdir, None).await?
            }
            "delete" => {
                let branch = require_name(name)?;
                run_git(&["branch", "-d", branch], &workdir, None).await?
            }
            other => {
                return Err(ToolError::InvalidParameters(format!(
                    "Unknown action: '{other}'. Use list, create, switch, or delete"
                )));
            }
        };

        let text = if output.stdout.trim().is_empty() {
            match action {
                "create" => format!("Branch '{}' created.", name.unwrap_or_default()),
                "delete" => format!("Branch '{}' deleted.", name.unwrap_or_default()),
                _ => "No branches found.".to_string(),
            }
        } else {
            output.stdout
        };

        Ok(ToolOutput::text(text, start.elapsed()))
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Container
    }

    fn risk_level_for(&self, params: &serde_json::Value) -> RiskLevel {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        match action {
            "list" => RiskLevel::Low,
            _ => RiskLevel::Medium,
        }
    }

    fn requires_approval(&self, params: &serde_json::Value) -> ApprovalRequirement {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list");
        match action {
            "list" => ApprovalRequirement::Never,
            _ => ApprovalRequirement::UnlessAutoApproved,
        }
    }

    fn requires_sanitization(&self) -> bool {
        false
    }
}

fn require_name(name: Option<&str>) -> Result<&str, ToolError> {
    match name {
        Some(n) if !n.trim().is_empty() => Ok(n),
        _ => Err(ToolError::InvalidParameters(
            "Missing required 'name' parameter for this branch action".into(),
        )),
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
    async fn test_branch_list() {
        let tool = GitBranchTool::new();
        let mut ctx = make_ctx();
        let result = tool.execute(serde_json::json!({}), &mut ctx).await;
        match result {
            Ok(output) => assert!(!output.result.as_str().unwrap_or_default().is_empty()),
            Err(ToolError::ExecutionFailed(msg)) if msg.contains("spawn git") => {}
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_create_missing_name() {
        let tool = GitBranchTool::new();
        let mut ctx = make_ctx();
        let result = tool
            .execute(serde_json::json!({"action": "create"}), &mut ctx)
            .await;
        assert!(matches!(result, Err(ToolError::InvalidParameters(_))));
    }

    #[test]
    fn test_risk_varies_by_action() {
        let tool = GitBranchTool::new();
        assert_eq!(
            tool.risk_level_for(&serde_json::json!({"action": "list"})),
            RiskLevel::Low
        );
        assert_eq!(
            tool.risk_level_for(&serde_json::json!({"action": "create"})),
            RiskLevel::Medium
        );
        assert_eq!(
            tool.risk_level_for(&serde_json::json!({"action": "delete"})),
            RiskLevel::Medium
        );
    }

    #[test]
    fn test_approval_varies_by_action() {
        let tool = GitBranchTool::new();
        assert_eq!(
            tool.requires_approval(&serde_json::json!({"action": "list"})),
            ApprovalRequirement::Never
        );
        assert_eq!(
            tool.requires_approval(&serde_json::json!({"action": "switch"})),
            ApprovalRequirement::UnlessAutoApproved
        );
    }
}
