//! Session fork tool for branching conversations.
//!
//! Creates a new thread by copying message history up to a specified turn,
//! allowing the user to explore alternative paths without losing the original
//! conversation.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::context::JobContext;
use crate::tools::tool::{ApprovalRequirement, Tool, ToolDomain, ToolError, ToolOutput};

/// Tool for forking a conversation thread at a specific turn.
///
/// The actual fork is performed by `Session::fork_thread()` in the session
/// layer. This tool validates parameters and returns the fork intent so
/// the dispatcher can execute the fork on the session.
pub struct SessionForkTool;

impl SessionForkTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SessionForkTool {
    fn name(&self) -> &str {
        "session_fork"
    }

    fn description(&self) -> &str {
        "Fork the current conversation from a specific turn, creating a new \
         independent thread with history copied up to that point. Use this to \
         explore alternative approaches without losing the original conversation."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "at_turn": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Turn number to fork from (0-indexed). History up to this turn is copied."
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for creating the fork."
                }
            },
            "required": ["at_turn"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &JobContext,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        let at_turn = params
            .get("at_turn")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                ToolError::InvalidParameters("'at_turn' must be a non-negative integer".into())
            })? as usize;

        let reason = params
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("User-initiated fork");

        // The actual fork is done by the session layer (dispatcher intercepts
        // this tool result and calls session.fork_thread()). Return the intent.
        let result = json!({
            "action": "fork",
            "at_turn": at_turn,
            "reason": reason,
            "thread_id": ctx.conversation_id.map(|id| id.to_string()),
            "message": format!(
                "Fork requested at turn {}. A new thread will be created with history up to that point.",
                at_turn
            )
        });

        Ok(ToolOutput::success(result, start.elapsed()))
    }

    fn requires_approval(&self, _params: &serde_json::Value) -> ApprovalRequirement {
        ApprovalRequirement::Never
    }

    fn domain(&self) -> ToolDomain {
        ToolDomain::Orchestrator
    }

    fn execution_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_ctx() -> JobContext {
        let mut ctx = JobContext::with_user("test-user", "test", "test");
        ctx.conversation_id = Some(uuid::Uuid::new_v4());
        ctx
    }

    #[tokio::test]
    async fn test_fork_valid() {
        let tool = SessionForkTool::new();
        let result = tool
            .execute(json!({"at_turn": 3}), &test_ctx())
            .await
            .expect("fork should succeed");
        assert_eq!(result.result["action"], "fork");
        assert_eq!(result.result["at_turn"], 3);
    }

    #[tokio::test]
    async fn test_fork_with_reason() {
        let tool = SessionForkTool::new();
        let result = tool
            .execute(
                json!({"at_turn": 1, "reason": "try alternative approach"}),
                &test_ctx(),
            )
            .await
            .expect("fork should succeed");
        assert_eq!(result.result["reason"], "try alternative approach");
    }

    #[tokio::test]
    async fn test_fork_at_zero() {
        let tool = SessionForkTool::new();
        let result = tool
            .execute(json!({"at_turn": 0}), &test_ctx())
            .await
            .expect("fork at 0 should succeed");
        assert_eq!(result.result["at_turn"], 0);
    }

    #[tokio::test]
    async fn test_fork_missing_at_turn() {
        let tool = SessionForkTool::new();
        let err = tool.execute(json!({}), &test_ctx()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_fork_negative_turn() {
        let tool = SessionForkTool::new();
        let err = tool
            .execute(json!({"at_turn": -1}), &test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }
}
