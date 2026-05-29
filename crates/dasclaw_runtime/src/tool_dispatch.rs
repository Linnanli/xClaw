//! Sequential tool dispatcher extracted from `HeadlessDelegate`.
//!
//! Encapsulates the per-iteration 5-step pipeline ADR-153 §1.1 locks
//! in (emit `ToolCallStart` → approval gate → pre-execute egress →
//! `ToolExecutor::execute` → post-execute egress + sanitizer + emit
//! `ToolResult`) so that:
//!
//! * `HeadlessDelegate::execute_tool_calls` reduces to a one-shot
//!   `SequentialDispatcher::dispatch` call, and
//! * a future `AgenticLoop` (ADR-160 §3 L1) can reuse the same
//!   implementation by injecting a `ToolDispatcher` trait whose
//!   default impl is this struct.
//!
//! This module is a pure refactor: behaviour is byte-identical to the
//! pre-extraction inline pipeline. The `agent.rs` tests cover the
//! shape end-to-end.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::agentic_loop::LoopOutcome;
use dasclaw_core::egress_apply::{EgressApply, apply_egress_decision};
use dasclaw_core::hooks::{EgressGate, EgressKind};
use dasclaw_core::messages::{ChatMessage, ToolCall};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::TokenUsage;
use dasclaw_core::traits::HostError;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AgentEvent;
use crate::agent::{ToolExecutor, ToolOutputSanitizer};
use crate::approval::{ApprovalDecision, ApprovalInbox, ApprovalPolicy};

/// Sentinel prefix for `LoopOutcome::Failure` strings produced when an
/// `ApprovalPolicy` flagged a tool call and the GUI replied
/// `ApprovalDecision::Reject` (issue #910, GUI blocker B4).
///
/// Decoded by `agent::map_outcome` back into
/// `AgentError::ApprovalRejected`.
pub(crate) const APPROVAL_REJECTED_SENTINEL_PREFIX: &str = "headless-agent::approval-rejected:";

#[derive(Serialize, Deserialize)]
pub(crate) struct RejectedPayload {
    pub(crate) tool_name: String,
    pub(crate) reason: Option<String>,
}

/// Outcome of `SequentialDispatcher::await_approval_for`.
enum ApprovalOutcome {
    NotRequired,
    Approved,
    Rejected { reason: Option<String> },
}

/// Best-effort emit; a closed receiver is not a loop-fatal error.
///
/// Shared between the `HeadlessDelegate` (which emits `FinishReason`
/// outside the tool pipeline) and `SequentialDispatcher` so the two
/// sites cannot drift.
pub(crate) async fn emit_event(tx: Option<&mpsc::Sender<AgentEvent>>, event: AgentEvent) {
    if let Some(tx) = tx {
        let _ = tx.send(event).await;
    }
}

/// Record the assistant's tool-call turn so the next LLM call sees the
/// canonical Anthropic-style assistant + tool_result pairing.
fn push_assistant_tool_calls(
    ctx: &mut ReasoningContext,
    content: Option<String>,
    tool_calls: &[ToolCall],
    usage: TokenUsage,
) {
    ctx.messages.push(
        ChatMessage::assistant_with_tool_calls(content, tool_calls.to_vec()).with_usage(usage),
    );
}

/// L2 seam (ADR-160 §3): drive a single agentic-loop iteration's
/// tool calls through whatever pipeline the host wires up.
///
/// The only in-tree impl today is [`SequentialDispatcher`], which
/// preserves the ADR-153 §1.1 5-step pipeline byte-for-byte. Future
/// `AgenticLoop`-level implementations (parallel dispatch, replay,
/// fakes for testing) plug in here without touching
/// `HeadlessDelegate` or `run_agentic_loop`.
#[async_trait]
pub trait ToolDispatcher: Send + Sync {
    /// Execute one iteration's worth of tool calls.
    ///
    /// Returning `Ok(Some(outcome))` short-circuits the agentic loop
    /// (e.g. approval rejection, fatal sandbox refusal). Returning
    /// `Ok(None)` lets the loop run another model turn.
    async fn dispatch(
        &self,
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        usage: TokenUsage,
        ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError>;
}

/// Sequential implementation of the ADR-153 §1.1 tool dispatch
/// pipeline. Executes the supplied `tool_calls` one after another;
/// short-circuits on approval rejection.
pub struct SequentialDispatcher {
    executor: Arc<dyn ToolExecutor>,
    egress: Arc<dyn EgressGate>,
    approval_policy: Arc<dyn ApprovalPolicy>,
    approval_inbox: ApprovalInbox,
    tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
}

impl SequentialDispatcher {
    /// Wire up a dispatcher with the runtime's shared collaborators.
    /// All `Arc` handles are cheap to clone, so a fresh dispatcher
    /// per loop iteration is acceptable.
    pub fn new(
        executor: Arc<dyn ToolExecutor>,
        egress: Arc<dyn EgressGate>,
        approval_policy: Arc<dyn ApprovalPolicy>,
        approval_inbox: ApprovalInbox,
        tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Self {
        Self {
            executor,
            egress,
            approval_policy,
            approval_inbox,
            tool_output_sanitizer,
            event_tx,
        }
    }
}

#[async_trait]
impl ToolDispatcher for SequentialDispatcher {
    async fn dispatch(
        &self,
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        usage: TokenUsage,
        ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        push_assistant_tool_calls(ctx, content, &tool_calls, usage);
        for call in &tool_calls {
            // Streaming hosts learn the tool name + arguments as soon as
            // the loop commits to invoking the tool, before any egress
            // gate runs. The post-egress sanitized payload is forwarded
            // in the matching `ToolResult` event below, so a redacted
            // arguments preview stays consistent with what the LLM sees
            // next iteration.
            self.emit(AgentEvent::ToolCallStart {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            })
            .await;

            // ADR-153 / issue #910 — GUI approval loop B4.
            match self.await_approval_for(call).await {
                ApprovalOutcome::NotRequired | ApprovalOutcome::Approved => {}
                ApprovalOutcome::Rejected { reason } => {
                    return Ok(Some(self.push_rejection(ctx, call, reason).await));
                }
            }

            // ADR-148 Layer B + ADR-153 §1.1 e8/A2:
            // Pre-execute egress gate scans serialised tool arguments.
            let mut args_buf =
                serde_json::to_string(&call.arguments).unwrap_or_else(|_| String::from("{}"));
            let kind = EgressKind::ToolExecution {
                tool: call.name.clone(),
            };
            let pre_label = format!("ToolExecution:{}", call.name);
            let decision = self.egress.check(&kind, &args_buf).await;
            if let EgressApply::Halt(reason) =
                apply_egress_decision(decision, &mut args_buf, &pre_label)
            {
                tracing::warn!(tool = %call.name, %reason, "egress gate blocked tool call");
                let error_body = format!("Error: {reason}");
                ctx.messages
                    .push(ChatMessage::tool_result(&call.id, &call.name, &error_body));
                self.emit(AgentEvent::ToolResult {
                    name: call.name.clone(),
                    content: error_body,
                    is_error: true,
                })
                .await;
                continue;
            }

            let result = self.executor.execute(call).await?;
            let executor_is_error = result.is_error;

            // ADR-148 Layer B + ADR-153 §1.1 e15/A9:
            // Post-execute egress gate sanitises tool output.
            let mut content_buf = result.content.clone();
            let post_label = format!("ToolOutput:{}", result.name);
            let decision = self
                .egress
                .check(&EgressKind::UserDisplay, &content_buf)
                .await;
            let (post_gate_content, gate_halted) = match apply_egress_decision(
                decision,
                &mut content_buf,
                &post_label,
            ) {
                EgressApply::Continue => (content_buf, false),
                EgressApply::Halt(reason) => {
                    tracing::warn!(tool = %result.name, %reason, "egress gate blocked tool output");
                    (format!("[redacted: {reason}]"), true)
                }
            };

            // ADR-153 §1.1 e22/B6: chain a second sanitizer seam after
            // the egress gate.
            let pushed_content = match self.tool_output_sanitizer.as_ref() {
                Some(sanitizer) => sanitizer.sanitize(&result.name, &post_gate_content),
                None => post_gate_content,
            };
            ctx.messages.push(ChatMessage::tool_result(
                &result.tool_call_id,
                &result.name,
                &pushed_content,
            ));
            self.emit(AgentEvent::ToolResult {
                name: result.name.clone(),
                content: pushed_content,
                is_error: executor_is_error || gate_halted,
            })
            .await;
        }
        Ok(None)
    }
}

impl SequentialDispatcher {
    async fn emit(&self, event: AgentEvent) {
        emit_event(self.event_tx.as_ref(), event).await;
    }

    async fn await_approval_for(&self, call: &ToolCall) -> ApprovalOutcome {
        let Some(request) = self.approval_policy.evaluate(call).await else {
            return ApprovalOutcome::NotRequired;
        };

        let request_id = Uuid::new_v4();
        let rx = self.approval_inbox.register(request_id);

        self.emit(AgentEvent::ApprovalNeeded {
            request_id,
            tool_name: call.name.clone(),
            tool_arguments: call.arguments.clone(),
            description: request.description,
            display_parameters: request.display_parameters,
            allow_always: request.allow_always,
        })
        .await;

        match rx.await {
            Ok(ApprovalDecision::Approve) | Ok(ApprovalDecision::ApproveAlways) => {
                ApprovalOutcome::Approved
            }
            Ok(ApprovalDecision::Reject { reason }) => ApprovalOutcome::Rejected { reason },
            Err(_) => {
                // Sender dropped without dispatching: forget the entry
                // so a late respond_to_approval call still hits
                // ApprovalDispatchError::Unknown rather than leaking the
                // pending request forever.
                self.approval_inbox.forget(request_id);
                ApprovalOutcome::Rejected {
                    reason: Some(String::from(
                        "approval channel closed before the GUI replied",
                    )),
                }
            }
        }
    }

    async fn push_rejection(
        &self,
        ctx: &mut ReasoningContext,
        call: &ToolCall,
        reason: Option<String>,
    ) -> LoopOutcome {
        let body = match reason.as_deref() {
            Some(r) => format!("Error: approval rejected: {r}"),
            None => String::from("Error: approval rejected"),
        };
        ctx.messages
            .push(ChatMessage::tool_result(&call.id, &call.name, &body).with_tool_error(true));
        self.emit(AgentEvent::ToolResult {
            name: call.name.clone(),
            content: body,
            is_error: true,
        })
        .await;
        let payload = RejectedPayload {
            tool_name: call.name.clone(),
            reason,
        };
        // `serde_json::to_string` on a two-field POD struct cannot fail
        // under normal conditions; the fallback keeps the call panic-free
        // (AGENTS.md production-code rule) and still lands on a
        // recognisable sentinel for `agent::map_outcome`.
        let encoded = serde_json::to_string(&payload)
            .unwrap_or_else(|_| String::from("{\"tool_name\":\"\"}"));
        LoopOutcome::Failure(format!("{APPROVAL_REJECTED_SENTINEL_PREFIX}{encoded}"))
    }
}
