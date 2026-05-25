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
use dasclaw_core::messages::FinishReason;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::AgentResponder;

/// `AgentResponder` that consumes a pre-canned queue of [`RespondOutput`]s
/// in FIFO order. Used by the W6.2 tests instead of [`dasclaw_cli::EchoResponder`]
/// because some cases need to verify the responder was **not** invoked
/// at all when a hook short-circuits the loop.
pub struct ScriptedResponder {
    queued: Mutex<Vec<RespondOutput>>,
    call_count: Mutex<usize>,
}

impl ScriptedResponder {
    /// Build a responder that replies once with a plain-text turn.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self::with_queue(vec![RespondOutput {
            result: RespondResult::Text(text.into()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        }])
    }

    /// Build a responder backed by an explicit FIFO queue.
    pub fn with_queue(turns: Vec<RespondOutput>) -> Self {
        Self {
            queued: Mutex::new(turns),
            call_count: Mutex::new(0),
        }
    }

    /// Number of times `respond` has been invoked.
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait]
impl AgentResponder for ScriptedResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        *self.call_count.lock().unwrap_or_else(|p| p.into_inner()) += 1;
        let mut q = self.queued.lock().unwrap_or_else(|p| p.into_inner());
        if q.is_empty() {
            return Err("ScriptedResponder queue exhausted".into());
        }
        Ok(q.remove(0))
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
