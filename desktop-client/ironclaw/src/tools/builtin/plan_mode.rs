//! Plan Mode tool for switching between planning and execution phases.
//!
//! In planning mode, the agent only analyses and produces a structured plan.
//! Write tools return dry-run previews instead of executing. The user must
//! approve the plan before execution begins.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::tools::tool::{ApprovalRequirement, Tool, ToolDomain, ToolError, ToolOutput};

/// Tool for toggling plan mode on a thread.
///
/// Actions:
/// - `toggle`  — switch plan mode on/off
/// - `status`  — query current plan mode state
/// - `submit`  — submit a structured plan for user review
pub struct PlanModeTool;

impl Default for PlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanModeTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for PlanModeTool {
    fn name(&self) -> &str {
        "plan_mode"
    }

    fn description(&self) -> &str {
        "Toggle planning mode. In planning mode only read-only tools execute; \
         write tools return dry-run previews. Use 'toggle' to switch modes, \
         'status' to check, or 'submit' to propose a structured plan."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["toggle", "status", "submit"],
                    "description": "Action to perform"
                },
                "plan": {
                    "type": "object",
                    "description": "Structured plan (required for 'submit' action)",
                    "properties": {
                        "goal": { "type": "string", "description": "Overall goal" },
                        "steps": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "description": { "type": "string" },
                                    "tool_name": { "type": "string" },
                                    "parameters": { "description": "Tool-specific parameters (freeform, schema varies by tool)" },
                                    "risk": { "type": "string", "enum": ["low", "medium", "high"] },
                                    "files": { "type": "array", "items": { "type": "string" } }
                                },
                                "required": ["description", "tool_name"]
                            }
                        },
                        "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
                    },
                    "required": ["goal", "steps"]
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &mut dyn dasclaw_runtime::JobContextCore,
    ) -> Result<ToolOutput, ToolError> {
        let start = std::time::Instant::now();

        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParameters("missing 'action'".into()))?;

        let result = match action {
            "toggle" => {
                // The actual toggle is handled by the dispatcher/session layer
                // via thread.toggle_plan_mode(). This tool returns the intent.
                json!({
                    "action": "toggle",
                    "message": "Plan mode toggle requested. The session layer will switch the thread state.",
                    "thread_id": ctx.conversation_id().map(|id| id.to_string())
                })
            }
            "status" => {
                // Status is read from thread metadata by the dispatcher.
                json!({
                    "action": "status",
                    "message": "Plan mode status query. Check thread.plan_mode for current state."
                })
            }
            "submit" => {
                let plan = params.get("plan").ok_or_else(|| {
                    ToolError::InvalidParameters("'submit' action requires 'plan' parameter".into())
                })?;

                let goal = plan
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidParameters("plan.goal is required".into()))?;

                let steps = plan
                    .get("steps")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        ToolError::InvalidParameters("plan.steps array is required".into())
                    })?;

                if steps.is_empty() {
                    return Err(ToolError::InvalidParameters(
                        "plan must have at least one step".into(),
                    ));
                }

                let confidence = plan
                    .get("confidence")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5);

                json!({
                    "action": "submit",
                    "goal": goal,
                    "steps_count": steps.len(),
                    "confidence": confidence,
                    "plan": plan,
                    "message": "Plan submitted for user review. Awaiting approval."
                })
            }
            other => {
                return Err(ToolError::InvalidParameters(format!(
                    "unknown action '{}', expected toggle/status/submit",
                    other
                )));
            }
        };

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
    use dasclaw_runtime::context::JobContext;
    use serde_json::json;

    fn test_ctx() -> JobContext {
        JobContext::with_user("test-user", "test", "test")
    }

    #[tokio::test]
    async fn test_plan_mode_toggle() {
        let tool = PlanModeTool::new();
        let result = tool
            .execute(json!({"action": "toggle"}), &mut test_ctx())
            .await
            .expect("toggle should succeed");
        assert_eq!(result.result["action"], "toggle");
    }

    #[tokio::test]
    async fn test_plan_mode_status() {
        let tool = PlanModeTool::new();
        let result = tool
            .execute(json!({"action": "status"}), &mut test_ctx())
            .await
            .expect("status should succeed");
        assert_eq!(result.result["action"], "status");
    }

    #[tokio::test]
    async fn test_plan_mode_submit_valid() {
        let tool = PlanModeTool::new();
        let params = json!({
            "action": "submit",
            "plan": {
                "goal": "Refactor module X",
                "steps": [
                    {
                        "description": "Read the file",
                        "tool_name": "read_file",
                        "risk": "low"
                    },
                    {
                        "description": "Edit the function",
                        "tool_name": "code_edit",
                        "risk": "medium",
                        "files": ["src/main.rs"]
                    }
                ],
                "confidence": 0.8
            }
        });
        let result = tool
            .execute(params, &mut test_ctx())
            .await
            .expect("submit should succeed");
        assert_eq!(result.result["action"], "submit");
        assert_eq!(result.result["steps_count"], 2);
        assert_eq!(result.result["confidence"], 0.8);
    }

    #[tokio::test]
    async fn test_plan_mode_submit_missing_plan() {
        let tool = PlanModeTool::new();
        let err = tool
            .execute(json!({"action": "submit"}), &mut test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_plan_mode_submit_empty_steps() {
        let tool = PlanModeTool::new();
        let params = json!({
            "action": "submit",
            "plan": {
                "goal": "Do nothing",
                "steps": [],
                "confidence": 0.5
            }
        });
        let err = tool.execute(params, &mut test_ctx()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_plan_mode_unknown_action() {
        let tool = PlanModeTool::new();
        let err = tool
            .execute(json!({"action": "explode"}), &mut test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_plan_mode_missing_action() {
        let tool = PlanModeTool::new();
        let err = tool.execute(json!({}), &mut test_ctx()).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidParameters(_)));
    }
}
