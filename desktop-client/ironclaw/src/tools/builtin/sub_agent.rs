//! Sub-Agent tool for spawning lightweight child agents.
//!
//! Sub-agents inherit partial context from the parent and execute with
//! restricted tool whitelists. They return a summary to the parent agent
//! rather than interacting with the user directly.
//!
//! Design: depth is limited to 1 (sub-agents cannot spawn further sub-agents).
//! Results are filtered through `ironclaw_safety` before injection into the
//! parent conversation.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::tool::{ApprovalRequirement, RiskLevel, Tool, ToolDomain, ToolError, ToolOutput};

/// Role a sub-agent can assume — determines its tool whitelist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubAgentRole {
    /// Read-only exploration: file reading, searching, code intelligence.
    Explore,
    /// Verification: read-only + readonly shell commands + git diff.
    Verify,
    /// Custom role with explicit tool whitelist.
    Custom(String),
}

impl SubAgentRole {
    /// Parse a role string.
    pub fn parse(s: &str) -> Result<Self, ToolError> {
        match s.to_lowercase().as_str() {
            "explore" => Ok(Self::Explore),
            "verify" => Ok(Self::Verify),
            other => Ok(Self::Custom(other.to_string())),
        }
    }

    /// Default tool whitelist for this role.
    pub fn default_tool_whitelist(&self) -> Vec<String> {
        match self {
            Self::Explore => vec![
                "read_file".to_string(),
                "grep_search".to_string(),
                "glob_search".to_string(),
                "list_dir".to_string(),
                "lsp_query".to_string(),
            ],
            Self::Verify => vec![
                "read_file".to_string(),
                "grep_search".to_string(),
                "glob_search".to_string(),
                "list_dir".to_string(),
                "lsp_query".to_string(),
                "shell".to_string(),
                "git_diff".to_string(),
            ],
            Self::Custom(_) => vec![],
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Explore => "explore",
            Self::Verify => "verify",
            Self::Custom(name) => name,
        }
    }

    /// Whether tools in this role are all read-only.
    pub fn is_readonly(&self) -> bool {
        matches!(self, Self::Explore)
    }
}

impl std::fmt::Display for SubAgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Maximum agent depth. Sub-agents are depth=1; they cannot spawn further sub-agents.
pub const MAX_SUB_AGENT_DEPTH: u16 = 1;

/// Default maximum turns for a sub-agent before it must return a summary.
pub const DEFAULT_MAX_TURNS: u16 = 10;

/// Tool for spawning a lightweight sub-agent.
///
/// The sub-agent runs in-process with a restricted tool whitelist determined
/// by its role. It inherits partial context from the parent (the goal message
/// and optionally recent conversation history).
///
/// The actual spawning is orchestrated by the dispatcher/job infrastructure.
/// This tool validates parameters and emits a structured intent.
pub struct SubAgentTool {
    /// Current depth (0 = main agent). Sub-agents at depth >= MAX_SUB_AGENT_DEPTH
    /// cannot spawn further sub-agents.
    current_depth: u16,
}

impl Default for SubAgentTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SubAgentTool {
    /// Create a new SubAgentTool at depth 0 (main agent).
    pub fn new() -> Self {
        Self { current_depth: 0 }
    }

    /// Create a SubAgentTool at a specific depth.
    pub fn at_depth(depth: u16) -> Self {
        Self {
            current_depth: depth,
        }
    }
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "sub_agent"
    }

    fn description(&self) -> &str {
        "Spawn a lightweight sub-agent to perform a focused task. \
         Available roles: 'explore' (read-only code search and analysis) \
         and 'verify' (read-only + shell + git diff). Sub-agents return \
         a summary and cannot spawn further sub-agents."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "role": {
                    "type": "string",
                    "enum": ["explore", "verify"],
                    "description": "Sub-agent role determining available tools"
                },
                "goal": {
                    "type": "string",
                    "description": "Task description for the sub-agent"
                },
                "max_turns": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 30,
                    "description": "Maximum turns before the sub-agent must summarize (default: 10)"
                },
                "inherit_context": {
                    "type": "boolean",
                    "description": "Whether to pass recent parent conversation as context (default: false)"
                },
                "tool_whitelist": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Additional tools to allow (merged with role defaults). Must be a subset of the role's allowed tools."
                }
            },
            "required": ["role", "goal"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        // Depth check: sub-agents cannot spawn further sub-agents
        if self.current_depth >= MAX_SUB_AGENT_DEPTH {
            return Err(ToolError::ExecutionFailed(format!(
                "sub-agent depth limit reached (current: {}, max: {}). \
                 Sub-agents cannot spawn further sub-agents.",
                self.current_depth, MAX_SUB_AGENT_DEPTH
            )));
        }

        let role_str = params
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("'role' is required".into()))?;

        let role = SubAgentRole::parse(role_str)?;

        let goal = params
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("'goal' is required".into()))?;

        if goal.trim().is_empty() {
            return Err(ToolError::InvalidParameters(
                "'goal' must not be empty".into(),
            ));
        }

        let max_turns = params
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .map(|v| v.min(30) as u16)
            .unwrap_or(DEFAULT_MAX_TURNS);

        let inherit_context = params
            .get("inherit_context")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let tool_whitelist = role.default_tool_whitelist();

        let result = json!({
            "action": "spawn_sub_agent",
            "role": role.as_str(),
            "goal": goal,
            "max_turns": max_turns,
            "inherit_context": inherit_context,
            "tool_whitelist": tool_whitelist,
            "depth": self.current_depth + 1,
            "thread_id": ctx.conversation_id().map(|id| id.to_string()),
            "message": format!(
                "Sub-agent ({}) spawned with goal: '{}'. Max {} turns, {} tools available.",
                role, goal, max_turns, tool_whitelist.len()
            )
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        // Spawning a sub-agent needs approval unless auto-approved,
        // because it consumes LLM tokens and execution time.
        ApprovalRequirement::UnlessAutoApproved
    }

    fn risk_level_for(&self, params: &serde_json::Value) -> RiskLevel {
        let role = params
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("explore");
        match role {
            "explore" => RiskLevel::Low,
            "verify" => RiskLevel::Medium,
            _ => RiskLevel::Medium,
        }
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Orchestrator
    }

    fn execution_timeout(&self) -> Duration {
        // Sub-agents can run for a while; 5 minutes max.
        Duration::from_secs(300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::JobContext;
    use serde_json::json;

    fn test_ctx() -> JobContext {
        let mut ctx = JobContext::with_user("test-user", "test", "test");
        ctx.conversation_id = Some(uuid::Uuid::new_v4());
        ctx
    }

    #[tokio::test]
    async fn test_spawn_explore_sub_agent() {
        let tool = SubAgentTool::new();
        let params = json!({"role": "explore", "goal": "Find all usages of foo()"});
        let result = tool
            .execute(params, &mut test_ctx())
            .await
            .expect("should succeed");
        assert_eq!(result.result["action"], "spawn_sub_agent");
        assert_eq!(result.result["role"], "explore");
        assert_eq!(result.result["depth"], 1);
        let whitelist = result.result["tool_whitelist"].as_array().expect("array");
        assert!(whitelist.iter().any(|v| v == "read_file"));
        assert!(whitelist.iter().any(|v| v == "grep_search"));
        // Explore should NOT have shell
        assert!(!whitelist.iter().any(|v| v == "shell"));
    }

    #[tokio::test]
    async fn test_spawn_verify_sub_agent() {
        let tool = SubAgentTool::new();
        let params = json!({"role": "verify", "goal": "Check tests pass"});
        let result = tool
            .execute(params, &mut test_ctx())
            .await
            .expect("should succeed");
        assert_eq!(result.result["role"], "verify");
        let whitelist = result.result["tool_whitelist"].as_array().expect("array");
        assert!(whitelist.iter().any(|v| v == "shell"));
        assert!(whitelist.iter().any(|v| v == "git_diff"));
    }

    #[tokio::test]
    async fn test_depth_limit_blocks_nested_spawn() {
        let tool = SubAgentTool::at_depth(1);
        let params = json!({"role": "explore", "goal": "nested search"});
        let err = tool.execute(params, &mut test_ctx()).await.unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
        let msg = err.to_string();
        assert!(
            msg.contains("depth limit"),
            "error should mention depth: {msg}"
        );
    }

    #[tokio::test]
    async fn test_missing_role() {
        let tool = SubAgentTool::new();
        let err = tool
            .execute(json!({"goal": "do stuff"}), &mut test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_missing_goal() {
        let tool = SubAgentTool::new();
        let err = tool
            .execute(json!({"role": "explore"}), &mut test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_empty_goal() {
        let tool = SubAgentTool::new();
        let err = tool
            .execute(json!({"role": "explore", "goal": "  "}), &mut test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_max_turns_clamped() {
        let tool = SubAgentTool::new();
        let params = json!({"role": "explore", "goal": "search", "max_turns": 100});
        let result = tool
            .execute(params, &mut test_ctx())
            .await
            .expect("should succeed");
        // Clamped to 30
        assert_eq!(result.result["max_turns"], 30);
    }

    #[tokio::test]
    async fn test_default_max_turns() {
        let tool = SubAgentTool::new();
        let params = json!({"role": "explore", "goal": "search"});
        let result = tool
            .execute(params, &mut test_ctx())
            .await
            .expect("should succeed");
        assert_eq!(result.result["max_turns"], DEFAULT_MAX_TURNS);
    }

    #[tokio::test]
    async fn test_explore_is_readonly() {
        assert!(SubAgentRole::Explore.is_readonly());
        assert!(!SubAgentRole::Verify.is_readonly());
    }

    #[tokio::test]
    async fn test_approval_requirement() {
        let tool = SubAgentTool::new();
        let params = json!({"role": "explore"});
        assert_eq!(
            tool.requires_approval(&params),
            ApprovalRequirement::UnlessAutoApproved
        );
    }

    #[tokio::test]
    async fn test_risk_level_by_role() {
        let tool = SubAgentTool::new();
        assert_eq!(
            tool.risk_level_for(&json!({"role": "explore"})),
            RiskLevel::Low
        );
        assert_eq!(
            tool.risk_level_for(&json!({"role": "verify"})),
            RiskLevel::Medium
        );
    }
}
