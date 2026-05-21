use serde_json::json;

use ironclaw::context::JobContext;
use ironclaw::tools::Tool;
use ironclaw::tools::builtin::{PlanModeTool, SessionForkTool, SubAgentTool};

fn test_ctx() -> JobContext {
    JobContext::with_user("test-user", "test", "test")
}

#[tokio::test]
async fn test_audit_plan_mode_invalid_input_does_not_echo_sensitive_plan() {
    let tool = PlanModeTool::new();
    let sensitive_goal = "refactor auth for key sk_live_1234567890abcdef";

    let error = tool
        .execute(
            json!({
                "action": "submit",
                "plan": {
                    "goal": sensitive_goal,
                    "steps": []
                }
            }),
            &mut test_ctx(),
        )
        .await
        .expect_err("empty steps should fail");

    let message = error.to_string();
    assert!(!message.contains(sensitive_goal));
    assert!(!message.contains("sk_live_1234567890abcdef"));
    assert!(message.contains("at least one step") || message.contains("Invalid"));
}

#[tokio::test]
async fn test_audit_session_fork_rejects_invalid_turn_without_content_leak() {
    let tool = SessionForkTool::new();
    let error = tool
        .execute(
            json!({
                "at_turn": -1,
                "reason": "fork around secret 330326199408015618"
            }),
            &mut test_ctx(),
        )
        .await
        .expect_err("negative turn should fail");

    let message = error.to_string();
    assert!(!message.contains("330326199408015618"));
    assert!(message.contains("at_turn") || message.contains("non-negative"));
}

#[tokio::test]
async fn test_audit_sub_agent_depth_limit_is_fail_safe() {
    let tool = SubAgentTool::at_depth(1);
    let error = tool
        .execute(
            json!({
                "role": "verify",
                "goal": "inspect prod secret token sk-prod-abcdef"
            }),
            &mut test_ctx(),
        )
        .await
        .expect_err("nested sub-agent should be rejected");

    let message = error.to_string();
    assert!(message.contains("depth limit"));
    assert!(!message.contains("sk-prod-abcdef"));
}

#[tokio::test]
async fn test_audit_sub_agent_missing_goal_does_not_leak_extra_fields() {
    let tool = SubAgentTool::new();
    let error = tool
        .execute(
            json!({
                "role": "verify",
                "notes": "secret: 13800138000"
            }),
            &mut test_ctx(),
        )
        .await
        .expect_err("missing goal should fail");

    let message = error.to_string();
    assert!(message.contains("goal"));
    assert!(!message.contains("13800138000"));
}
