//! ADR-160 §3 L1: `AgenticLoop` as a concrete struct (W6.4; updated W10.1).
//!
//! W10.1 collapsed the prior `LoopDelegate` adapter — the new
//! [`dasclaw_core::agentic_loop::run_agentic_loop`] takes the same four
//! seams ([`AgentResponder`], [`ToolDispatcher`], cancellation token,
//! event channel) directly, so the `LoopAdapter` shim is gone. This
//! struct now exists purely as the L1 façade hosts that don't need a
//! [`crate::Agent`] facade can target, and to own the per-run
//! collaborators for as long as the run is in flight.

use std::sync::Arc;

use dasclaw_core::agentic_loop::{
    AgentEvent, AgentResponder, AgenticLoopConfig, LoopOutcome, ModelCallMode, ToolDispatcher,
    run_agentic_loop,
};
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::traits::HostError;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// L1 seam (ADR-160 §3): concrete agentic-loop driver.
///
/// Owns the per-run collaborators (`responder`, `dispatcher`,
/// `cancellation_token`, `event_tx`) and forwards to
/// [`run_agentic_loop`]. Hosts that don't need a [`crate::Agent`]
/// facade can build an `AgenticLoop` directly.
pub struct AgenticLoop {
    responder: Arc<dyn AgentResponder>,
    dispatcher: Option<Arc<dyn ToolDispatcher>>,
    cancellation_token: Option<CancellationToken>,
    model_call_mode: ModelCallMode,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
}

impl AgenticLoop {
    /// Wire up a loop with the runtime's shared collaborators.
    ///
    /// Pass `dispatcher = None` to keep the agent in text-only mode;
    /// any model-emitted tool call then resolves to the
    /// `TOOLS_NOT_SUPPORTED_REASON` sentinel, matching the pre-W6.4
    /// pre-W10.1 `HeadlessDelegate` contract.
    #[must_use]
    pub fn new(
        responder: Arc<dyn AgentResponder>,
        dispatcher: Option<Arc<dyn ToolDispatcher>>,
        cancellation_token: Option<CancellationToken>,
        model_call_mode: ModelCallMode,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Self {
        Self {
            responder,
            dispatcher,
            cancellation_token,
            model_call_mode,
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
        run_agentic_loop(
            self.responder.as_ref(),
            self.dispatcher.as_deref(),
            self.cancellation_token.as_ref(),
            self.model_call_mode,
            self.event_tx.as_ref(),
            ctx,
            loop_config,
            hooks,
        )
        .await
    }
}
