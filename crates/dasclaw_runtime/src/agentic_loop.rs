//! ADR-160 §3 L1: `AgenticLoop` as a concrete struct (W6.4).
//!
//! Wraps [`dasclaw_core::agentic_loop::run_agentic_loop`] behind a
//! struct that owns the seams the loop needs at runtime: the LLM
//! responder, an optional [`ToolDispatcher`] (L2 seam), a cancellation
//! token, and a streaming event channel. The struct is the new
//! single-entry point that [`crate::Agent`] drives; the prior
//! private `HeadlessDelegate` is now an internal adapter
//! ([`LoopAdapter`]) whose `LoopDelegate` impl is a byte-identical
//! move of the pre-W6.4 inline implementation.
//!
//! ## Why a struct, not a free function
//!
//! The pre-W6.4 path built a fresh `HeadlessDelegate` per
//! `run_in_context_inner` call and re-constructed
//! [`SequentialDispatcher`] inside `execute_tool_calls` on **every
//! iteration**. The struct gives us:
//!
//! - one place that owns the dispatcher for the lifetime of the run
//!   (built once at `Agent::run_in_context_inner`, not per iteration);
//! - a stable injection point for fakes / parallel dispatchers /
//!   replay engines without touching the agentic-loop crate;
//! - the L1 seam ADR-160 §3 asks for, so future hosts can build
//!   loops without going through `Agent` at all.
//!
//! ## Scope (W6.4)
//!
//! - Byte-equivalent behaviour with the pre-W6.4 `HeadlessDelegate`.
//! - No public-API churn for `Agent` / `AgentBuilder` callers.
//! - `desktop-client/ironclaw` delegates (ChatDelegate / JobDelegate /
//!   ContainerDelegate) are untouched — they keep calling
//!   `run_agentic_loop` directly.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::ToolCall;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agent::{AgentEvent, AgentResponder, TOOLS_NOT_SUPPORTED_REASON};
use crate::tool_dispatch::{ToolDispatcher, emit_event};

/// L1 seam (ADR-160 §3): concrete agentic-loop driver.
///
/// Owns the per-run collaborators (`responder`, `dispatcher`,
/// `cancellation_token`, `event_tx`) and forwards to
/// [`run_agentic_loop`] via an internal [`LoopAdapter`]. Hosts that
/// don't need a [`crate::Agent`] facade can build an `AgenticLoop`
/// directly.
pub struct AgenticLoop {
    responder: Arc<dyn AgentResponder>,
    dispatcher: Option<Arc<dyn ToolDispatcher>>,
    cancellation_token: Option<CancellationToken>,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
}

impl AgenticLoop {
    /// Wire up a loop with the runtime's shared collaborators.
    ///
    /// Pass `dispatcher = None` to keep the agent in text-only mode;
    /// any model-emitted tool call then resolves to the
    /// [`TOOLS_NOT_SUPPORTED_REASON`] sentinel, matching the pre-W6.4
    /// `HeadlessDelegate` contract.
    #[must_use]
    pub fn new(
        responder: Arc<dyn AgentResponder>,
        dispatcher: Option<Arc<dyn ToolDispatcher>>,
        cancellation_token: Option<CancellationToken>,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Self {
        Self {
            responder,
            dispatcher,
            cancellation_token,
            event_tx,
        }
    }

    /// Drive the agentic loop to completion.
    ///
    /// `ctx` must already be seeded with the system prompt, advertised
    /// tools and the latest user message (the [`crate::Agent`] facade
    /// does this for callers).
    pub async fn run(
        &self,
        ctx: &mut ReasoningContext,
        loop_config: &AgenticLoopConfig,
        hooks: &HookBundle,
    ) -> Result<LoopOutcome, HostError> {
        let adapter = LoopAdapter { inner: self };
        run_agentic_loop(&adapter, ctx, loop_config, hooks).await
    }
}

/// Internal `LoopDelegate` adapter. Byte-identical move of the
/// pre-W6.4 `HeadlessDelegate::LoopDelegate` impl, refactored to
/// borrow its dependencies from an enclosing [`AgenticLoop`].
struct LoopAdapter<'a> {
    inner: &'a AgenticLoop,
}

impl<'a> LoopAdapter<'a> {
    /// Best-effort emit; a closed receiver is not a loop-fatal error.
    async fn emit(&self, event: AgentEvent) {
        emit_event(self.inner.event_tx.as_ref(), event).await;
    }
}

#[async_trait]
impl<'a> LoopDelegate for LoopAdapter<'a> {
    async fn check_signals(&self) -> LoopSignal {
        // Cancellation token short-circuits the responder hook: a
        // cancelled token always wins even if the responder would
        // return `Continue`.
        if let Some(token) = self.inner.cancellation_token.as_ref()
            && token.is_cancelled()
        {
            return LoopSignal::Stop;
        }
        self.inner.responder.check_signals().await
    }

    async fn before_llm_call(
        &self,
        ctx: &mut ReasoningContext,
        iteration: usize,
    ) -> Option<LoopOutcome> {
        self.inner.responder.before_llm_call(ctx, iteration).await
    }

    async fn call_llm(
        &self,
        ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Result<RespondOutput, HostError> {
        // Streaming and non-streaming routes split at the responder
        // seam: when an event channel is wired we go through
        // `respond_streaming`, which the default trait impl falls back
        // to `respond` for adapters that don't override it. Either way
        // we forward the trailing `FinishReason` so a GUI knows the
        // iteration boundary even when the model emitted no text.
        let output = match self.inner.event_tx.as_ref() {
            Some(tx) => {
                self.inner
                    .responder
                    .respond_streaming(ctx, tx.clone())
                    .await?
            }
            None => self.inner.responder.respond(ctx).await?,
        };
        self.emit(AgentEvent::FinishReason(output.finish_reason))
            .await;
        Ok(output)
    }

    async fn handle_text_response(
        &self,
        text: &str,
        metadata: ResponseMetadata,
        usage: TokenUsage,
        ctx: &mut ReasoningContext,
    ) -> TextAction {
        self.inner
            .responder
            .handle_text_response(text, metadata, usage, ctx)
            .await
    }

    async fn execute_tool_calls(
        &self,
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        usage: TokenUsage,
        ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        // No dispatcher wired → emit the sentinel and let `map_outcome`
        // raise `AgentError::ToolsNotSupported`. This keeps the A1
        // contract intact when callers use text-only agents.
        let Some(dispatcher) = self.inner.dispatcher.as_ref() else {
            return Ok(Some(LoopOutcome::Failure(
                TOOLS_NOT_SUPPORTED_REASON.to_string(),
            )));
        };
        dispatcher.dispatch(tool_calls, content, usage, ctx).await
    }

    async fn on_tool_intent_nudge(&self, text: &str, ctx: &mut ReasoningContext) {
        self.inner.responder.on_tool_intent_nudge(text, ctx).await;
    }

    async fn after_iteration(&self, iteration: usize) {
        self.inner.responder.after_iteration(iteration).await;
    }
}
