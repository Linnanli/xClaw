//! Issue #910 (GUI blocker B4): integration tests for the GUI
//! approval loop.
//!
//! Covers the four named requirements:
//!
//! - `req_dasclaw_runtime_b4_approve_resumes_tool_execution`
//! - `req_dasclaw_runtime_b4_reject_returns_approval_rejected_error`
//! - `req_dasclaw_runtime_b4_unknown_request_id_rejected`
//! - `req_dasclaw_runtime_b4_no_policy_no_approval_events`
//!
//! Plus a small wire-shape lock for `AgentEvent::ApprovalNeeded` so the
//! GUI's TypeScript discriminated union doesn't drift unnoticed.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dasclaw_core::messages::{FinishReason, ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{
    Agent, AgentError, AgentEvent, AgentResponder, ApprovalDecision, ApprovalDispatchError,
    ApprovalPolicy, ApprovalRequest, ToolExecutor,
};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test scaffolding
// ---------------------------------------------------------------------------

/// Mock responder driven by a fixed sequence of pre-built outputs.
struct ScriptedResponder {
    script: Mutex<Vec<RespondOutput>>,
}

impl ScriptedResponder {
    fn new(script: Vec<RespondOutput>) -> Self {
        Self {
            script: Mutex::new(script),
        }
    }
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let mut s = self.script.lock().await;
        if s.is_empty() {
            return Err("script exhausted".into());
        }
        Ok(s.remove(0))
    }
}

/// Executor that returns a fixed `ToolResult` and counts invocations.
struct CountingExecutor {
    calls: Mutex<Vec<ToolCall>>,
}

impl CountingExecutor {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    async fn call_count(&self) -> usize {
        self.calls.lock().await.len()
    }
}

#[async_trait]
impl ToolExecutor for CountingExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        self.calls.lock().await.push(call.clone());
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: format!("ran {}", call.name),
            is_error: false,
        })
    }
}

/// Policy that always requires approval, regardless of tool.
struct AlwaysApprovePolicy;

#[async_trait]
impl ApprovalPolicy for AlwaysApprovePolicy {
    async fn evaluate(&self, call: &ToolCall) -> Option<ApprovalRequest> {
        Some(ApprovalRequest {
            description: format!("approve call to {}", call.name),
            display_parameters: call.arguments.clone(),
            allow_always: true,
        })
    }
}

fn tool_call_output(name: &str, id: &str) -> RespondOutput {
    RespondOutput {
        result: RespondResult::ToolCalls {
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: serde_json::json!({"path": "/tmp/x"}),
                reasoning: None,
            }],
            content: None,
        },
        usage: TokenUsage::default(),
        finish_reason: FinishReason::ToolUse,
        metadata: ResponseMetadata::default(),
    }
}

fn text_output(s: &str) -> RespondOutput {
    RespondOutput {
        result: RespondResult::Text(s.to_string()),
        usage: TokenUsage::default(),
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    }
}

fn dummy_tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: "test tool".into(),
        parameters: serde_json::json!({"type": "object"}),
    }
}

// ---------------------------------------------------------------------------
// req_dasclaw_runtime_b4_approve_resumes_tool_execution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn req_dasclaw_runtime_b4_approve_resumes_tool_execution() {
    let executor = Arc::new(CountingExecutor::new());
    let agent = Arc::new(
        Agent::builder()
            .responder(ScriptedResponder::new(vec![
                tool_call_output("bash", "call_1"),
                text_output("done"),
            ]))
            .tool_executor(Arc::clone(&executor) as Arc<dyn ToolExecutor>)
            .tools(vec![dummy_tool("bash")])
            .approval_policy(Arc::new(AlwaysApprovePolicy))
            .build()
            .expect("build agent"),
    );

    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);
    let agent_for_drain = Arc::clone(&agent);

    // GUI simulator: on the first ApprovalNeeded, dispatch Approve.
    let drain = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(ev) = event_rx.recv().await {
            if let AgentEvent::ApprovalNeeded { request_id, .. } = &ev {
                let id = *request_id;
                agent_for_drain
                    .respond_to_approval(id, ApprovalDecision::Approve)
                    .expect("approval dispatch succeeds");
            }
            events.push(ev);
        }
        events
    });

    let mut ctx = ReasoningContext::new();
    let outcome = agent
        .run_in_context_streaming(&mut ctx, "hi", event_tx)
        .await
        .expect("run completes");
    assert_eq!(outcome, "done");

    let events = drain.await.expect("drain task joins");
    let approval_count = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ApprovalNeeded { .. }))
        .count();
    assert_eq!(approval_count, 1, "exactly one approval prompt expected");

    let tool_result_errors = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ToolResult { is_error: true, .. }))
        .count();
    assert_eq!(
        tool_result_errors, 0,
        "approve path must not surface error tool results"
    );
    assert_eq!(executor.call_count().await, 1, "tool must run once");
}

// ---------------------------------------------------------------------------
// req_dasclaw_runtime_b4_reject_returns_approval_rejected_error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn req_dasclaw_runtime_b4_reject_returns_approval_rejected_error() {
    let executor = Arc::new(CountingExecutor::new());
    let agent = Arc::new(
        Agent::builder()
            .responder(ScriptedResponder::new(vec![tool_call_output(
                "bash", "call_1",
            )]))
            .tool_executor(Arc::clone(&executor) as Arc<dyn ToolExecutor>)
            .tools(vec![dummy_tool("bash")])
            .approval_policy(Arc::new(AlwaysApprovePolicy))
            .build()
            .expect("build agent"),
    );

    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);
    let agent_for_drain = Arc::clone(&agent);

    let drain = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(ev) = event_rx.recv().await {
            if let AgentEvent::ApprovalNeeded { request_id, .. } = &ev {
                let id = *request_id;
                agent_for_drain
                    .respond_to_approval(
                        id,
                        ApprovalDecision::Reject {
                            reason: Some("nope".into()),
                        },
                    )
                    .expect("approval dispatch succeeds");
            }
            events.push(ev);
        }
        events
    });

    let mut ctx = ReasoningContext::new();
    let err = agent
        .run_in_context_streaming(&mut ctx, "hi", event_tx)
        .await
        .expect_err("rejected approval must surface as error");

    match err {
        AgentError::ApprovalRejected { tool_name, reason } => {
            assert_eq!(tool_name, "bash");
            assert_eq!(reason.as_deref(), Some("nope"));
        }
        other => panic!("expected ApprovalRejected, got {other:?}"),
    }

    let events = drain.await.expect("drain joins");
    let error_results: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolResult {
                name,
                content,
                is_error: true,
            } => Some((name.clone(), content.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        error_results.len(),
        1,
        "expected exactly one error tool_result event, got {error_results:?}"
    );
    assert_eq!(error_results[0].0, "bash");
    assert!(
        error_results[0].1.contains("nope"),
        "tool error body should embed rejection reason: {}",
        error_results[0].1
    );
    assert_eq!(
        executor.call_count().await,
        0,
        "executor must not run when approval is rejected"
    );
}

// ---------------------------------------------------------------------------
// req_dasclaw_runtime_b4_unknown_request_id_rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn req_dasclaw_runtime_b4_unknown_request_id_rejected() {
    let agent = Agent::builder()
        .responder(ScriptedResponder::new(vec![text_output("noop")]))
        .build()
        .expect("build agent");

    let err = agent
        .respond_to_approval(Uuid::new_v4(), ApprovalDecision::Approve)
        .expect_err("unknown id must error");
    assert!(matches!(err, ApprovalDispatchError::Unknown(_)));
}

// ---------------------------------------------------------------------------
// req_dasclaw_runtime_b4_no_policy_no_approval_events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn req_dasclaw_runtime_b4_no_policy_no_approval_events() {
    // No .approval_policy() call → NoApprovalPolicy default → zero
    // ApprovalNeeded events even for tool calls.
    let executor = Arc::new(CountingExecutor::new());
    let agent = Agent::builder()
        .responder(ScriptedResponder::new(vec![
            tool_call_output("bash", "call_1"),
            text_output("done"),
        ]))
        .tool_executor(Arc::clone(&executor) as Arc<dyn ToolExecutor>)
        .tools(vec![dummy_tool("bash")])
        .build()
        .expect("build agent");

    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(32);
    let drain = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(ev) = event_rx.recv().await {
            events.push(ev);
        }
        events
    });

    let mut ctx = ReasoningContext::new();
    let outcome = agent
        .run_in_context_streaming(&mut ctx, "hi", event_tx)
        .await
        .expect("run completes");
    assert_eq!(outcome, "done");

    // Allow drain to finish.
    let events = tokio::time::timeout(Duration::from_secs(2), drain)
        .await
        .expect("drain finishes")
        .expect("drain joins");

    let approval_events = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::ApprovalNeeded { .. }))
        .count();
    assert_eq!(
        approval_events, 0,
        "default NoApprovalPolicy must not emit ApprovalNeeded events"
    );
    assert_eq!(executor.call_count().await, 1, "tool should still run");
}

// ---------------------------------------------------------------------------
// Wire-shape lock for AgentEvent::ApprovalNeeded
// ---------------------------------------------------------------------------

#[test]
fn agent_event_approval_needed_serde_roundtrip() {
    let id = Uuid::new_v4();
    let original = AgentEvent::ApprovalNeeded {
        request_id: id,
        tool_name: "bash".into(),
        tool_arguments: serde_json::json!({"cmd": "ls"}),
        description: "needs approval".into(),
        display_parameters: serde_json::json!({"cmd": "ls"}),
        allow_always: true,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    assert!(json.contains("approval_needed") || json.contains("ApprovalNeeded"));
    let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, original);
}
