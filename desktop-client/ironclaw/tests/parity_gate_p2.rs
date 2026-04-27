//! Claude Code Parity Gate — P2 验收测试
//!
//! **FP-023 ~ FP-030**: P2 功能 Parity 场景
//!
//! 覆盖:
//! - Plan Mode (PlanModeTool, ThreadState::Planning, dry-run interception)
//! - Session Fork (fork_thread, independent evolution)
//! - Sub-Agent (SubAgentTool, depth limit, role whitelists)
//!
//! 全部通过才算 P2 Gate cleared。

use ironclaw::agent::session::{PendingPlan, PlanStep, Session, Thread, ThreadState};
use ironclaw::context::JobContext;
use ironclaw::tools::builtin::sub_agent::SubAgentRole;
use ironclaw::tools::builtin::{PlanModeTool, SessionForkTool, SubAgentTool};
use ironclaw::tools::{ApprovalRequirement, RiskLevel, Tool};

use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn make_ctx() -> JobContext {
    let mut ctx = JobContext::default();
    ctx.conversation_id = Some(Uuid::new_v4());
    ctx
}

fn session_with_turns(n: usize) -> (Session, Uuid) {
    let mut session = Session::new("test-user");
    let thread = session.create_thread();
    let thread_id = thread.id;
    for i in 0..n {
        thread.start_turn(format!("message {}", i));
        thread.complete_turn(format!("response {}", i));
    }
    (session, thread_id)
}

// ═══════════════════════════════════════════════════════════════════════
// FP-023: Plan Mode — toggle switches thread to Planning state
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_023_plan_mode_toggle_switches_to_planning() {
    let mut session = Session::new("u1");
    let thread = session.create_thread();
    assert_eq!(thread.state, ThreadState::Idle);
    assert!(!thread.is_plan_mode());

    // Toggle ON
    let enabled = thread.toggle_plan_mode();
    assert!(enabled);
    assert!(thread.is_plan_mode());
    assert_eq!(thread.state, ThreadState::Planning);

    // Toggle OFF
    let disabled = thread.toggle_plan_mode();
    assert!(!disabled);
    assert!(!thread.is_plan_mode());
    assert_eq!(thread.state, ThreadState::Idle);
}

// ═══════════════════════════════════════════════════════════════════════
// FP-023b: Plan Mode tool returns valid response for toggle/status/submit
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_023b_plan_mode_tool_actions() {
    let tool = PlanModeTool::new();
    let ctx = make_ctx();

    // Toggle
    let result = tool
        .execute(serde_json::json!({"action": "toggle"}), &ctx)
        .await
        .expect("toggle");
    assert_eq!(result.result["action"], "toggle");

    // Status
    let result = tool
        .execute(serde_json::json!({"action": "status"}), &ctx)
        .await
        .expect("status");
    assert_eq!(result.result["action"], "status");

    // Submit plan
    let result = tool
        .execute(
            serde_json::json!({
                "action": "submit",
                "plan": {
                    "goal": "Refactor the auth module",
                    "steps": [
                        {"description": "Read auth.rs", "tool_name": "read_file", "risk": "low"},
                        {"description": "Edit handler", "tool_name": "code_edit", "risk": "medium", "files": ["src/auth.rs"]}
                    ],
                    "confidence": 0.85
                }
            }),
            &ctx,
        )
        .await
        .expect("submit");
    assert_eq!(result.result["action"], "submit");
    assert_eq!(result.result["steps_count"], 2);
}

// ═══════════════════════════════════════════════════════════════════════
// FP-024: Plan approval switches to Processing + returns the plan
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_024_plan_approval_switches_to_processing() {
    let mut session = Session::new("u1");
    let thread = session.create_thread();
    let _thread_id = thread.id;

    // Enter planning mode
    thread.toggle_plan_mode();
    assert_eq!(thread.state, ThreadState::Planning);

    // Set a pending plan
    let plan = PendingPlan {
        plan_id: Uuid::new_v4(),
        goal: "Refactor".into(),
        steps: vec![PlanStep {
            step: 1,
            description: "Read file".into(),
            tool_name: "read_file".into(),
            parameters: serde_json::json!({}),
            risk: "low".into(),
            files: vec![],
            executed: false,
            result: None,
        }],
        confidence: 0.9,
        created_at: chrono::Utc::now(),
    };
    thread.set_pending_plan(plan);
    assert!(thread.pending_plan.is_some());

    // Approve
    let approved = thread.approve_plan();
    assert!(approved.is_some());
    assert_eq!(thread.state, ThreadState::Processing);
    assert!(!thread.is_plan_mode());
    assert!(thread.pending_plan.is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// FP-025: Fork from Turn 3 creates new thread with correct history
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_025_fork_creates_new_thread_with_history() {
    let (mut session, source_id) = session_with_turns(5);

    // Fork at turn 3 — copies turns 0, 1, 2
    let new_id = session
        .fork_thread(source_id, 3)
        .expect("fork should succeed");

    // Verify new thread exists
    let forked = session.threads.get(&new_id).expect("forked thread");
    assert_eq!(forked.turns.len(), 3);
    assert_eq!(forked.state, ThreadState::Idle);
    assert_eq!(forked.forked_from, Some(source_id));
    assert_eq!(forked.fork_point, Some(3));

    // Verify turn content was copied
    assert_eq!(forked.turns[0].user_input, "message 0");
    assert_eq!(forked.turns[2].user_input, "message 2");

    // Source thread is unmodified
    let source = session.threads.get(&source_id).unwrap();
    assert_eq!(source.turns.len(), 5);

    // Active thread switched to fork
    assert_eq!(session.active_thread, Some(new_id));
}

// ═══════════════════════════════════════════════════════════════════════
// FP-025b: Fork at turn 0 creates empty thread
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_025b_fork_at_zero_creates_empty_thread() {
    let (mut session, source_id) = session_with_turns(3);

    let new_id = session
        .fork_thread(source_id, 0)
        .expect("fork at 0 should succeed");

    let forked = session.threads.get(&new_id).unwrap();
    assert!(forked.turns.is_empty());
    assert_eq!(forked.forked_from, Some(source_id));
}

// ═══════════════════════════════════════════════════════════════════════
// FP-025c: Fork out of range returns None
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_025c_fork_out_of_range_returns_none() {
    let (mut session, source_id) = session_with_turns(3);

    assert!(session.fork_thread(source_id, 10).is_none());
    assert!(session.fork_thread(Uuid::new_v4(), 1).is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// FP-026: Forked thread evolves independently
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_026_forked_thread_independent_evolution() {
    let (mut session, source_id) = session_with_turns(3);

    let fork_id = session.fork_thread(source_id, 2).unwrap();

    // Add a new turn to the fork
    {
        let forked = session.threads.get_mut(&fork_id).unwrap();
        forked.start_turn("fork-specific message");
        forked.complete_turn("fork-specific response");
        assert_eq!(forked.turns.len(), 3); // 2 copied + 1 new
    }

    // Source is unmodified
    let source = session.threads.get(&source_id).unwrap();
    assert_eq!(source.turns.len(), 3);

    // The fork's last turn is different from source's
    let forked = session.threads.get(&fork_id).unwrap();
    let fork_last = &forked.turns[2].user_input;
    let source_last = &source.turns[2].user_input;
    assert_ne!(fork_last, source_last);
}

// ═══════════════════════════════════════════════════════════════════════
// FP-026b: Fork does not copy pending approvals
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_026b_fork_does_not_copy_pending_approvals() {
    let mut session = Session::new("u1");
    let thread = session.create_thread();
    let thread_id = thread.id;

    // Set up pending approval on source thread
    thread.start_turn("do something dangerous");
    thread.complete_turn("ok");
    thread.pending_approval = Some(ironclaw::agent::session::PendingApproval {
        request_id: Uuid::new_v4(),
        tool_name: "shell".into(),
        parameters: serde_json::json!({"command": "rm -rf /"}),
        display_parameters: serde_json::json!({"command": "rm -rf /"}),
        description: "dangerous".into(),
        tool_call_id: "tc_1".into(),
        context_messages: vec![],
        deferred_tool_calls: vec![],
        user_timezone: None,
        allow_always: false,
    });

    // Fork
    let fork_id = session.fork_thread(thread_id, 1).unwrap();
    let forked = session.threads.get(&fork_id).unwrap();

    // Pending approval NOT copied
    assert!(forked.pending_approval.is_none());
    assert!(!forked.plan_mode);
}

// ═══════════════════════════════════════════════════════════════════════
// FP-027: Explore Sub-Agent has readonly tool whitelist
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_027_explore_sub_agent_readonly_whitelist() {
    let tool = SubAgentTool::new();
    let ctx = make_ctx();

    let result = tool
        .execute(
            serde_json::json!({"role": "explore", "goal": "Find all usages of handle_request()"}),
            &ctx,
        )
        .await
        .expect("explore should succeed");

    let whitelist = result.result["tool_whitelist"]
        .as_array()
        .expect("whitelist should be array");

    // Must have readonly tools
    let names: Vec<&str> = whitelist.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"grep_search"));
    assert!(names.contains(&"glob_search"));
    assert!(names.contains(&"list_dir"));
    assert!(names.contains(&"lsp_query"));

    // Must NOT have write tools
    assert!(!names.contains(&"shell"));
    assert!(!names.contains(&"write_file"));
    assert!(!names.contains(&"code_edit"));
    assert!(!names.contains(&"git_commit"));

    // Role should be readonly
    assert!(SubAgentRole::Explore.is_readonly());
}

// ═══════════════════════════════════════════════════════════════════════
// FP-028: Verify Sub-Agent includes readonly shell + git_diff
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_028_verify_sub_agent_has_shell_and_git_diff() {
    let tool = SubAgentTool::new();
    let ctx = make_ctx();

    let result = tool
        .execute(
            serde_json::json!({"role": "verify", "goal": "Check test output"}),
            &ctx,
        )
        .await
        .expect("verify should succeed");

    let whitelist = result.result["tool_whitelist"]
        .as_array()
        .expect("whitelist");
    let names: Vec<&str> = whitelist.iter().filter_map(|v| v.as_str()).collect();

    // Verify has shell + git_diff
    assert!(names.contains(&"shell"), "verify should include shell");
    assert!(
        names.contains(&"git_diff"),
        "verify should include git_diff"
    );
    // Plus all explore tools
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"grep_search"));

    // Verify is NOT readonly
    assert!(!SubAgentRole::Verify.is_readonly());
}

// ═══════════════════════════════════════════════════════════════════════
// FP-029: Sub-Agent depth=1 limit blocks nested spawn
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_029_sub_agent_depth_limit() {
    // Agent at depth 0 can spawn
    let tool_d0 = SubAgentTool::new();
    let ctx = make_ctx();
    let result = tool_d0
        .execute(
            serde_json::json!({"role": "explore", "goal": "search"}),
            &ctx,
        )
        .await
        .expect("depth 0 should succeed");
    assert_eq!(result.result["depth"], 1);

    // Agent at depth 1 CANNOT spawn
    let tool_d1 = SubAgentTool::at_depth(1);
    let err = tool_d1
        .execute(
            serde_json::json!({"role": "explore", "goal": "nested search"}),
            &ctx,
        )
        .await;
    assert!(err.is_err(), "depth 1 should fail");
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("depth limit"), "error: {msg}");
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030: Sub-Agent approval requirement + risk levels
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030_sub_agent_approval_and_risk() {
    let tool = SubAgentTool::new();

    // Approval: unless auto-approved (costs tokens)
    assert_eq!(
        tool.requires_approval(&serde_json::json!({})),
        ApprovalRequirement::UnlessAutoApproved
    );

    // Risk: explore = Low, verify = Medium
    assert_eq!(
        tool.risk_level_for(&serde_json::json!({"role": "explore"})),
        RiskLevel::Low
    );
    assert_eq!(
        tool.risk_level_for(&serde_json::json!({"role": "verify"})),
        RiskLevel::Medium
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030b: SessionForkTool validates parameters
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030b_session_fork_tool_validation() {
    let tool = SessionForkTool::new();
    let ctx = make_ctx();

    // Valid fork
    let result = tool
        .execute(serde_json::json!({"at_turn": 3}), &ctx)
        .await
        .expect("valid fork");
    assert_eq!(result.result["action"], "fork");
    assert_eq!(result.result["at_turn"], 3);

    // Missing at_turn → error
    let err = tool.execute(serde_json::json!({}), &ctx).await;
    assert!(err.is_err());

    // Approval: Never (session fork is a navigational action)
    assert_eq!(
        tool.requires_approval(&serde_json::json!({})),
        ApprovalRequirement::Never
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030c: PlanModeTool rejects invalid input
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030c_plan_mode_rejects_invalid() {
    let tool = PlanModeTool::new();
    let ctx = make_ctx();

    // Missing action
    assert!(tool.execute(serde_json::json!({}), &ctx).await.is_err());

    // Unknown action
    assert!(
        tool.execute(serde_json::json!({"action": "explode"}), &ctx)
            .await
            .is_err()
    );

    // Submit without plan
    assert!(
        tool.execute(serde_json::json!({"action": "submit"}), &ctx)
            .await
            .is_err()
    );

    // Submit with empty steps
    assert!(
        tool.execute(
            serde_json::json!({"action": "submit", "plan": {"goal": "x", "steps": []}}),
            &ctx
        )
        .await
        .is_err()
    );
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030d: SubAgentRole defaults
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030d_sub_agent_role_whitelists() {
    let explore = SubAgentRole::Explore;
    let verify = SubAgentRole::Verify;
    let custom = SubAgentRole::Custom("helper".into());

    // Explore: 5 readonly tools
    let ew = explore.default_tool_whitelist();
    assert_eq!(ew.len(), 5);
    assert!(ew.contains(&"read_file".to_string()));
    assert!(!ew.contains(&"shell".to_string()));

    // Verify: 7 tools (explore + shell + git_diff)
    let vw = verify.default_tool_whitelist();
    assert_eq!(vw.len(), 7);
    assert!(vw.contains(&"shell".to_string()));
    assert!(vw.contains(&"git_diff".to_string()));

    // Custom: empty by default
    assert!(custom.default_tool_whitelist().is_empty());

    // Display
    assert_eq!(explore.as_str(), "explore");
    assert_eq!(verify.as_str(), "verify");
    assert_eq!(custom.as_str(), "helper");
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030e: Thread plan mode integration — toggle + pending plan lifecycle
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030e_thread_plan_lifecycle() {
    let mut thread = Thread::new(Uuid::new_v4());

    // Initial state
    assert!(!thread.is_plan_mode());
    assert_eq!(thread.state, ThreadState::Idle);

    // Toggle on
    assert!(thread.toggle_plan_mode());
    assert_eq!(thread.state, ThreadState::Planning);

    // Set pending plan
    let plan = PendingPlan {
        plan_id: Uuid::new_v4(),
        goal: "test goal".into(),
        steps: vec![PlanStep {
            step: 1,
            description: "step 1".into(),
            tool_name: "read_file".into(),
            parameters: serde_json::json!({}),
            risk: "low".into(),
            files: vec![],
            executed: false,
            result: None,
        }],
        confidence: 0.7,
        created_at: chrono::Utc::now(),
    };
    thread.set_pending_plan(plan);
    assert!(thread.pending_plan.is_some());

    // Approve → Processing
    let approved = thread.approve_plan();
    assert!(approved.is_some());
    assert_eq!(thread.state, ThreadState::Processing);
    assert!(!thread.is_plan_mode());
    assert!(thread.pending_plan.is_none());

    // Approve when no plan → None
    let none = thread.approve_plan();
    assert!(none.is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// FP-030f: Toggle off clears pending plan
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fp_030f_toggle_off_clears_plan() {
    let mut thread = Thread::new(Uuid::new_v4());
    thread.toggle_plan_mode();

    // Set a pending plan
    thread.set_pending_plan(PendingPlan {
        plan_id: Uuid::new_v4(),
        goal: "test".into(),
        steps: vec![PlanStep {
            step: 1,
            description: "s".into(),
            tool_name: "t".into(),
            parameters: serde_json::json!({}),
            risk: "low".into(),
            files: vec![],
            executed: false,
            result: None,
        }],
        confidence: 0.5,
        created_at: chrono::Utc::now(),
    });

    // Toggle off → plan cleared
    assert!(!thread.toggle_plan_mode());
    assert!(thread.pending_plan.is_none());
    assert_eq!(thread.state, ThreadState::Idle);
}
