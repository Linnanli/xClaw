//! Headless agent starter (doc 53 PR-1, ADR-153 §3 L1 facade).
//!
//! End-to-end demo for the four pieces a GUI host wires together when
//! it embeds [`dasclaw_runtime::Agent`]:
//!
//! 1. an [`AgentResponder`] (here a scripted fake — real hosts plug in
//!    [`dasclaw_runtime::LlmProviderResponder`] over `dasclaw_llm_provider`);
//! 2. a [`ToolExecutor`] (here a one-tool echo executor);
//! 3. an [`ApprovalPolicy`] that flags risky tool calls;
//! 4. the [`AgentEvent`] stream + [`Agent::respond_to_approval`] reply
//!    loop that B4 added in issue #910.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p dasclaw_cli --example headless_agent_starter
//! ```
//!
//! Expected output ends with `final text: ok (approved)` for the
//! happy path and an `ApprovalRejected` error for the reject path.

use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use async_trait::async_trait;
use dasclaw_core::messages::{FinishReason, ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{
    Agent, AgentBuilder, AgentError, AgentEvent, AgentResponder, ApprovalDecision, ApprovalPolicy,
    ApprovalRequest, ToolExecutor,
};
use serde_json::json;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    println!("--- demo 1: approve the only tool call ---");
    let approved = run_demo(ApprovalDecision::Approve).await?;
    println!("final text: {approved} (approved)");

    println!("\n--- demo 2: reject the only tool call ---");
    match run_demo(ApprovalDecision::Reject {
        reason: Some("policy violation".into()),
    })
    .await
    {
        Err(AgentError::ApprovalRejected { tool_name, reason }) => {
            let reason = reason.unwrap_or_else(|| "<none>".into());
            println!("rejected: tool={tool_name} reason={reason}");
        }
        Ok(text) => anyhow::bail!("expected ApprovalRejected, got Ok({text})"),
        Err(other) => anyhow::bail!("expected ApprovalRejected, got {other:?}"),
    }
    Ok(())
}

/// Build the agent, spawn it, and drive the approval loop with `decision`.
async fn run_demo(decision: ApprovalDecision) -> Result<String, AgentError> {
    let agent = Arc::new(build_agent()?);
    let (tx, mut rx) = mpsc::channel::<AgentEvent>(16);

    let agent_for_task = Arc::clone(&agent);
    let join = tokio::spawn(async move { agent_for_task.run_streaming("compute 2+2", tx).await });

    // Pump events until the task finishes. The only event we *act* on
    // is `ApprovalNeeded`; the rest are just logged so the demo shows
    // what a GUI would render.
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::ApprovalNeeded {
                request_id,
                tool_name,
                description,
                ..
            } => {
                println!("approval needed: tool={tool_name} description={description}");
                agent
                    .respond_to_approval(request_id, decision.clone())
                    .map_err(|e| {
                        AgentError::Responder(host_err(format!("dispatch failed: {e}")))
                    })?;
            }
            other => println!("event: {other:?}"),
        }
    }

    join.await
        .map_err(|e| AgentError::Responder(host_err(format!("join: {e}"))))?
}

/// Wire the four headless pieces together via [`AgentBuilder`].
fn build_agent() -> Result<Agent, AgentError> {
    AgentBuilder::default()
        .responder(ScriptedResponder::default())
        .tool_executor(EchoExecutor)
        .tools(vec![ToolDefinition {
            name: "compute".into(),
            description: "Compute a math expression".into(),
            parameters: json!({
                "type": "object",
                "properties": {"expr": {"type": "string"}},
                "required": ["expr"],
            }),
        }])
        .approval_policy(Arc::new(AlwaysAskPolicy))
        .build()
}

/// Two-turn script: turn 1 calls `compute`, turn 2 emits final text `ok`.
#[derive(Default)]
struct ScriptedResponder {
    turn: Mutex<usize>,
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let next = {
            let mut turn = self.turn.lock().map_err(|e| host_err(e.to_string()))?;
            let value = *turn;
            *turn += 1;
            value
        };
        let result = match next {
            0 => RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: "call-1".into(),
                    name: "compute".into(),
                    arguments: json!({"expr": "2+2"}),
                    reasoning: None,
                }],
                content: None,
            },
            _ => RespondResult::Text("ok".into()),
        };
        Ok(RespondOutput {
            result,
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}

/// One-tool executor: every call returns `{"echo": <arguments>}`.
struct EchoExecutor;

#[async_trait]
impl ToolExecutor for EchoExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: json!({"echo": call.arguments}).to_string(),
            is_error: false,
        })
    }
}

/// Policy that asks for approval on *every* tool call.
struct AlwaysAskPolicy;

#[async_trait]
impl ApprovalPolicy for AlwaysAskPolicy {
    async fn evaluate(&self, call: &ToolCall) -> Option<ApprovalRequest> {
        Some(ApprovalRequest {
            description: format!("run `{}`", call.name),
            display_parameters: call.arguments.clone(),
            allow_always: false,
        })
    }
}

fn host_err<M: Into<String>>(msg: M) -> HostError {
    Box::<dyn std::error::Error + Send + Sync>::from(msg.into())
}
