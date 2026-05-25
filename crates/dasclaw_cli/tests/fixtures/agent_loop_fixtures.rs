//! Shared fixtures for W6.3 agent-loop integration tests
//! (ADR-153 §1.1 B1/B2/B5/B7 — e17/e18/e21/e23).
//!
//! Designed for **zero file overlap** with `safety_fixtures.rs` (W6.2)
//! so the two PR chains can be reviewed in parallel without rebase
//! conflicts. The shapes intentionally mirror the unit-test
//! `ScriptedResponder` already living inside `dasclaw_runtime::agent`,
//! re-exported here in the public-API form callers will use.
//!
//! Allowed `dead_code` because each `tests/*.rs` integration binary
//! only consumes a subset of these helpers.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dasclaw_core::hooks::{EgressDecision, EgressGate, EgressKind, RedactionStats};
use dasclaw_core::messages::{FinishReason, ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{AgentResponder, ToolExecutor};
use serde_json::Value as JsonValue;

/// Convenience constructor for a `Text(...)` `RespondOutput` with
/// [`FinishReason::Stop`].
pub fn text_turn(text: impl Into<String>) -> RespondOutput {
    RespondOutput {
        result: RespondResult::Text(text.into()),
        usage: TokenUsage::default(),
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    }
}

/// Convenience constructor for a `ToolCalls { … }` `RespondOutput` with
/// [`FinishReason::ToolUse`]. Single-call helper — most agent-loop tests
/// only need one tool call per turn.
pub fn tool_call_turn(
    tool: impl Into<String>,
    call_id: impl Into<String>,
    arguments: JsonValue,
) -> RespondOutput {
    RespondOutput {
        result: RespondResult::ToolCalls {
            tool_calls: vec![ToolCall {
                id: call_id.into(),
                name: tool.into(),
                arguments,
                reasoning: None,
            }],
            content: None,
        },
        usage: TokenUsage::default(),
        finish_reason: FinishReason::ToolUse,
        metadata: ResponseMetadata::default(),
    }
}

/// Mock [`AgentResponder`] driven by a fixed `Vec<RespondOutput>` queue.
///
/// Each `respond` call pops the front and returns it. Once the queue is
/// empty, returns `HostError("script exhausted")` so test failures point
/// to the off-by-one cause directly.
pub struct ScriptedResponder {
    script: Mutex<Vec<RespondOutput>>,
}

impl ScriptedResponder {
    pub fn with_queue(script: Vec<RespondOutput>) -> Self {
        Self {
            script: Mutex::new(script),
        }
    }
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let mut s = self.script.lock().expect("scripted responder mutex");
        if s.is_empty() {
            return Err("script exhausted".into());
        }
        Ok(s.remove(0))
    }
}

/// Per-tool call record so tests can verify both call ordering and
/// argument payloads without re-deriving them from log strings.
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    pub name: String,
    pub call_id: String,
    pub arguments: JsonValue,
}

/// [`ToolExecutor`] that records every invocation and routes to a
/// per-tool reply table. Falls back to a generic "ok" reply when the
/// tool name is unknown so tests don't have to enumerate every variant.
pub struct RecordingToolExecutor {
    calls: Arc<Mutex<Vec<ToolInvocation>>>,
    replies: Vec<(String, String)>,
}

impl RecordingToolExecutor {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            replies: Vec::new(),
        }
    }

    /// Bind `tool_name → content` so the first call to that tool returns
    /// `content`. Subsequent calls also return the same content; tests
    /// that need step-wise behaviour should use [`FlakyToolExecutor`].
    pub fn with_reply(mut self, tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        self.replies.push((tool_name.into(), content.into()));
        self
    }

    pub fn calls(&self) -> Arc<Mutex<Vec<ToolInvocation>>> {
        Arc::clone(&self.calls)
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().expect("recording executor mutex").len()
    }
}

impl Default for RecordingToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolExecutor for RecordingToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        self.calls
            .lock()
            .expect("recording executor mutex")
            .push(ToolInvocation {
                name: call.name.clone(),
                call_id: call.id.clone(),
                arguments: call.arguments.clone(),
            });
        let reply = self
            .replies
            .iter()
            .find(|(name, _)| name == &call.name)
            .map(|(_, body)| body.clone())
            .unwrap_or_else(|| format!("ok:{}", call.name));
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: reply,
            is_error: false,
        })
    }
}

/// [`ToolExecutor`] that returns `is_error=true` on the first call to
/// any tool listed in `error_first`, then succeeds afterwards. Used by
/// e18 to drive the model's recovery branch.
pub struct FlakyToolExecutor {
    calls: Arc<Mutex<Vec<ToolInvocation>>>,
    error_first: Vec<String>,
    error_message: String,
    success_content: String,
}

impl FlakyToolExecutor {
    pub fn new(
        error_first: impl IntoIterator<Item = impl Into<String>>,
        error_message: impl Into<String>,
        success_content: impl Into<String>,
    ) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            error_first: error_first.into_iter().map(Into::into).collect(),
            error_message: error_message.into(),
            success_content: success_content.into(),
        }
    }

    pub fn calls(&self) -> Arc<Mutex<Vec<ToolInvocation>>> {
        Arc::clone(&self.calls)
    }

    pub fn call_count_for(&self, tool: &str) -> usize {
        self.calls
            .lock()
            .expect("flaky executor mutex")
            .iter()
            .filter(|c| c.name == tool)
            .count()
    }
}

#[async_trait]
impl ToolExecutor for FlakyToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let mut log = self.calls.lock().expect("flaky executor mutex");
        log.push(ToolInvocation {
            name: call.name.clone(),
            call_id: call.id.clone(),
            arguments: call.arguments.clone(),
        });
        let prior_attempts = log.iter().filter(|c| c.name == call.name).count() - 1;
        let should_fail = prior_attempts == 0 && self.error_first.iter().any(|t| t == &call.name);
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: if should_fail {
                self.error_message.clone()
            } else {
                self.success_content.clone()
            },
            is_error: should_fail,
        })
    }
}

/// [`EgressGate`] that returns `Block { reason }` when an
/// [`EgressKind::LlmRequest`] payload contains any of the configured
/// bait phrases (case-insensitive). All other kinds Pass through.
/// Used by e23 to validate the refusal path: model never speaks, no
/// tool is invoked, and the CLI surfaces a human-readable reason.
pub struct SecretBaitLlmGate {
    pub bait: Vec<String>,
    pub reason: String,
}

impl SecretBaitLlmGate {
    pub fn new(
        bait: impl IntoIterator<Item = impl Into<String>>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            bait: bait.into_iter().map(|s| s.into().to_lowercase()).collect(),
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl EgressGate for SecretBaitLlmGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        if !matches!(kind, EgressKind::LlmRequest) {
            return EgressDecision::Allow;
        }
        let lower = payload.to_lowercase();
        if self.bait.iter().any(|b| lower.contains(b)) {
            EgressDecision::Block {
                reason: self.reason.clone(),
                stats: RedactionStats::default(),
            }
        } else {
            EgressDecision::Allow
        }
    }
}

/// Convenience: build an empty `Vec<ToolDefinition>` for tests that
/// don't care about schema validation. Kept here so each test file
/// doesn't re-import the type.
pub fn no_tool_defs() -> Vec<ToolDefinition> {
    Vec::new()
}
