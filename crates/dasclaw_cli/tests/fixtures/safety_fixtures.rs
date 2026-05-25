//! Shared test fixtures for the W6.2 P0 safety e2e suite (ADR-153 §1.2).
//!
//! Each integration test under `tests/safety_*_e2e.rs` includes this
//! module via `#[path = "fixtures/safety_fixtures.rs"] mod safety_fixtures;`.
//!
//! Fixtures intentionally avoid `unwrap()` / `panic!()` in production-style
//! code paths; the only `expect`s are inside async test setup helpers
//! that fail the test outright when a precondition is violated.
#![allow(dead_code)] // Each test binary uses only a subset.

use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use dasclaw_core::hooks::{EgressDecision, EgressGate, EgressKind};
use dasclaw_core::messages::{ChatMessage, FinishReason, ToolCall, ToolResult};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{AgentResponder, ToolExecutor};

/// `AgentResponder` that consumes a pre-canned queue of [`RespondOutput`]s
/// in FIFO order. Used by the W6.2 tests instead of [`dasclaw_cli::EchoResponder`]
/// because some cases need to verify the responder was **not** invoked
/// at all when a hook short-circuits the loop.
pub struct ScriptedResponder {
    queued: Mutex<Vec<RespondOutput>>,
    call_count: Mutex<usize>,
    snapshots: Mutex<Vec<Vec<ChatMessage>>>,
}

impl ScriptedResponder {
    /// Build a responder that replies once with a plain-text turn.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self::with_queue(vec![text_turn(text)])
    }

    /// Build a responder backed by an explicit FIFO queue.
    pub fn with_queue(turns: Vec<RespondOutput>) -> Self {
        Self {
            queued: Mutex::new(turns),
            call_count: Mutex::new(0),
            snapshots: Mutex::new(Vec::new()),
        }
    }

    /// Number of times `respond` has been invoked.
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A clone of the `ctx.messages` observed at the start of each
    /// `respond` invocation. Element `i` is the snapshot taken on the
    /// `i`-th call. Used by e15 to assert that the tool_result the
    /// model sees on its second turn has been sanitised by the
    /// post-execute egress gate.
    pub fn snapshot(&self, index: usize) -> Option<Vec<ChatMessage>> {
        self.snapshots
            .lock()
            .ok()
            .and_then(|s| s.get(index).cloned())
    }
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        if let Ok(mut snaps) = self.snapshots.lock() {
            snaps.push(ctx.messages.clone());
        }
        *self.call_count.lock().unwrap_or_else(|p| p.into_inner()) += 1;
        let mut q = self.queued.lock().unwrap_or_else(|p| p.into_inner());
        if q.is_empty() {
            return Err("ScriptedResponder queue exhausted".into());
        }
        Ok(q.remove(0))
    }
}

/// Helper: build a plain-text `RespondOutput` turn.
pub fn text_turn(text: impl Into<String>) -> RespondOutput {
    RespondOutput {
        result: RespondResult::Text(text.into()),
        usage: TokenUsage::default(),
        finish_reason: FinishReason::Stop,
        metadata: ResponseMetadata::default(),
    }
}

/// Helper: build a tool-call `RespondOutput` turn. `arguments_json` is a
/// `serde_json::Value`, matching `ToolCall::arguments`.
pub fn tool_call_turn(
    tool: impl Into<String>,
    call_id: impl Into<String>,
    arguments: serde_json::Value,
) -> RespondOutput {
    let call = ToolCall {
        id: call_id.into(),
        name: tool.into(),
        arguments,
        reasoning: None,
    };
    RespondOutput {
        result: RespondResult::ToolCalls {
            tool_calls: vec![call],
            content: None,
        },
        usage: TokenUsage::default(),
        finish_reason: FinishReason::ToolUse,
        metadata: ResponseMetadata::default(),
    }
}

/// `EgressGate` that unconditionally returns [`EgressDecision::Block`].
/// Used by e10 to verify that the CLI surfaces gate decisions as
/// [`dasclaw_runtime::AgentError::LoopFailure`] (fail-closed seam).
pub struct BlockingEgressGate {
    pub reason: String,
}

impl BlockingEgressGate {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl EgressGate for BlockingEgressGate {
    async fn check(&self, _kind: &EgressKind, _payload: &str) -> EgressDecision {
        EgressDecision::Block {
            reason: self.reason.clone(),
            stats: Default::default(),
        }
    }
}

/// One observed gate invocation: `(kind, payload-snapshot, decision)`.
pub type EgressCall = (EgressKind, String, EgressDecision);

/// `EgressGate` that wraps a delegate and records every invocation for
/// later inspection. Records are appended even when the delegate returns
/// [`EgressDecision::Block`], so tests can assert which `EgressKind`
/// variant was the one that tripped.
pub struct RecordingEgressGate {
    inner: Arc<dyn EgressGate>,
    calls: Arc<Mutex<Vec<EgressCall>>>,
}

impl RecordingEgressGate {
    pub fn wrap(inner: Arc<dyn EgressGate>) -> Self {
        Self {
            inner,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Handle to the call log. Cheap to clone — shares the underlying
    /// `Mutex<Vec<_>>` with the gate.
    pub fn calls(&self) -> Arc<Mutex<Vec<EgressCall>>> {
        Arc::clone(&self.calls)
    }
}

#[async_trait]
impl EgressGate for RecordingEgressGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        let decision = self.inner.check(kind, payload).await;
        if let Ok(mut log) = self.calls.lock() {
            log.push((kind.clone(), payload.to_string(), decision.clone()));
        }
        decision
    }
}

/// `EgressGate` that returns [`EgressDecision::Block`] **only** when the
/// kind is [`EgressKind::ToolExecution`]. All other kinds Pass through.
/// Used by e8 to verify that a Block at the pre-execute tool-arg gate
/// prevents `ToolExecutor::execute` from being invoked.
pub struct BlockingToolExecGate {
    pub reason: String,
}

impl BlockingToolExecGate {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

#[async_trait]
impl EgressGate for BlockingToolExecGate {
    async fn check(&self, kind: &EgressKind, _payload: &str) -> EgressDecision {
        match kind {
            EgressKind::ToolExecution { .. } => EgressDecision::Block {
                reason: self.reason.clone(),
                stats: Default::default(),
            },
            _ => EgressDecision::Allow,
        }
    }
}

/// `EgressGate` that returns [`EgressDecision::Redact`] **only** when the
/// kind is [`EgressKind::UserDisplay`] *and* the payload contains
/// `trigger`. All other kinds, and UserDisplay payloads that don't match
/// the trigger, Pass through. Used by e15 to verify that the
/// post-execute tool-output gate rewrites injection content before it
/// lands in `ReasoningContext` while leaving the model's clean final
/// reply unchanged.
pub struct SanitizingDisplayGate {
    pub trigger: String,
    pub sanitized: String,
}

impl SanitizingDisplayGate {
    pub fn new(trigger: impl Into<String>, sanitized: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            sanitized: sanitized.into(),
        }
    }
}

#[async_trait]
impl EgressGate for SanitizingDisplayGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        match kind {
            EgressKind::UserDisplay if payload.contains(&self.trigger) => EgressDecision::Redact {
                sanitized: self.sanitized.clone(),
                stats: Default::default(),
            },
            _ => EgressDecision::Allow,
        }
    }
}

/// `ToolExecutor` that records every invocation and returns a pre-canned
/// [`ToolResult`] (default: empty success). Used by e8 to assert that the
/// executor was **not** invoked when the pre-execute egress gate Blocks.
pub struct RecordingToolExecutor {
    calls: Arc<Mutex<Vec<ToolCall>>>,
    response_content: String,
}

impl RecordingToolExecutor {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response_content: "ok".into(),
        }
    }

    pub fn with_content(content: impl Into<String>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response_content: content.into(),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().map(|c| c.len()).unwrap_or(0)
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
        if let Ok(mut log) = self.calls.lock() {
            log.push(call.clone());
        }
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: self.response_content.clone(),
            is_error: false,
        })
    }
}

/// `ToolExecutor` that always returns a [`ToolResult`] whose content
/// embeds a prompt-injection payload. Used by e15 to verify that the
/// post-execute `EgressKind::UserDisplay` gate rewrites the content
/// before it reaches the next LLM turn.
pub struct LeakyToolExecutor {
    pub content: String,
}

impl LeakyToolExecutor {
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: content.into() }
    }
}

#[async_trait]
impl ToolExecutor for LeakyToolExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content: self.content.clone(),
            is_error: false,
        })
    }
}
