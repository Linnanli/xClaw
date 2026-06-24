//! Unified agentic loop engine.
//!
//! Ported from ironclaw `agent/agentic_loop.rs` as part of Phase 3 Step D-4,
//! then collapsed in W10.1 (issue #982) onto a thinner two-seam contract:
//! the loop drives an [`AgentResponder`] for LLM I/O and an optional
//! [`ToolDispatcher`] for tool execution. The prior `LoopDelegate` trait
//! that lumped both seams together is gone — see the W10 PR series for the
//! migration trail.
//!
//! The engine runs the core LLM call → tool execution → result processing →
//! context update → repeat cycle. Three consumers (chat dispatcher, job
//! worker, container runtime) plug in by implementing the two narrow
//! traits and (in the desktop case) building their own `AgenticLoop`
//! wrapper in `dasclaw_runtime`.
//!
//! ## Route B (scope-narrowing vs. ironclaw's pre-port version)
//!
//! The upstream engine took a `&Reasoning` and passed it through every
//! `call_llm` invocation. That made the engine aware of ironclaw's concrete
//! LLM engine and forced the `dasclaw_core` port to invent a trait facade.
//!
//! Route B removes the `reasoning` parameter entirely: each responder owns
//! whatever LLM engine it needs internally. The engine only sees
//! [`RespondOutput`] values — it doesn't know an "LLM" exists.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::egress_apply::{EgressApply, apply_egress_decision};
use crate::hooks::{EgressKind, HookBundle};
use crate::intent::{TOOL_INTENT_NUDGE, TRUNCATED_TOOL_CALL_NOTICE, llm_signals_tool_intent};
use crate::messages::{ChatMessage, FinishReason, Role, ToolCall};
use crate::reasoning_ctx::ReasoningContext;
use crate::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use crate::session::PendingApproval;
use crate::traits::HostError;

/// Signal from the responder indicating how the loop should proceed.
pub enum LoopSignal {
    /// Continue normally.
    Continue,
    /// Stop the loop gracefully.
    Stop,
    /// Inject a user message into context and continue.
    InjectMessage(String),
}

/// Outcome of a text response from the LLM.
pub enum TextAction {
    /// Return this as the final loop result.
    Return(LoopOutcome),
    /// Continue the loop (text was handled but loop should proceed).
    Continue,
}

/// Final outcome of the agentic loop.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum LoopOutcome {
    /// Completed with a text response.
    Response(String),
    /// Loop was stopped by a signal.
    Stopped,
    /// Max iterations exceeded.
    MaxIterations,
    /// Loop terminated early with a clear failure reason.
    Failure(String),
    /// A tool requires user approval before continuing (chat delegate only).
    NeedApproval(Box<PendingApproval>),
}

/// Configuration for the agentic loop.
pub struct AgenticLoopConfig {
    pub max_iterations: usize,
    pub enable_tool_intent_nudge: bool,
    pub max_tool_intent_nudges: u32,
}

impl Default for AgenticLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            enable_tool_intent_nudge: true,
            max_tool_intent_nudges: 2,
        }
    }
}

/// Sentinel surfaced as `LoopOutcome::Failure` when the model emits a
/// tool call but no [`ToolDispatcher`] was wired up. The runtime maps
/// this back to `AgentError::ToolsNotSupported` (see
/// `dasclaw_runtime::AgentError`).
pub const TOOLS_NOT_SUPPORTED_REASON: &str = "headless-agent::tools-not-supported";

/// Final output returned by a completed agent run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRunOutput {
    /// User-visible assistant text.
    pub text: String,
    /// Token usage for the LLM response that produced this final text.
    #[serde(default)]
    pub usage: TokenUsage,
}

/// Provider call mode for the next model request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCallMode {
    /// Request a whole response from the provider.
    Invoke,
    /// Request a native provider stream.
    Stream,
}

/// Per-iteration responder policy.
#[derive(Clone)]
pub struct AgentCallPolicy {
    pub model_call_mode: ModelCallMode,
    pub event_tx: Option<mpsc::Sender<AgentEvent>>,
}

/// Streaming event emitted by the loop and forwarded to GUI / CLI hosts
/// (issue #908, GUI blocker B2).
///
/// One event per observable step in the agentic loop:
///
/// - `TextChunk` — a fragment of user-visible model output as it arrives
///   from the LLM. For providers without true token streaming the chunk
///   arrives in a single piece; consumers should not depend on a particular
///   chunk size.
/// - `ReasoningSummaryChunk` — a fragment of model reasoning summary.
///   This is not assistant message text and must never be mirrored into
///   user-visible delta streams.
/// - `ToolCallStart` — the model asked to invoke a tool; emitted before
///   tool execution.
/// - `ToolResult` — the tool finished; payload is the sanitized content
///   that the **next** LLM iteration will see in its `tool_result` block
///   (post-egress, post-sanitizer).
/// - `FinishReason` — one per LLM iteration boundary, carrying the
///   model's stop reason. Use it to render "stopped", "needs tool", etc.
///   in a GUI.
/// - `ApprovalNeeded` — the configured approval policy flagged a tool
///   call as needing human approval; the loop has paused until the host
///   replies with a matching decision.
///
/// Wire format is adjacent-tagged JSON
/// (`{"kind": "text_chunk", "data": "..."}`) so a TypeScript discriminated
/// union renders directly from `serde_json::to_string(&event)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A fragment of user-visible model text output.
    TextChunk(String),
    /// A fragment of model reasoning summary.
    ReasoningSummaryChunk(String),
    /// Provider reported a new reasoning summary part.
    ReasoningSummaryPartAdded {
        item_id: Option<String>,
        summary_index: i64,
    },
    /// Provider emitted raw reasoning text.
    ReasoningRawTextChunk {
        item_id: Option<String>,
        content_index: i64,
        delta: String,
    },
    /// Provider emitted an incremental plan delta.
    PlanDelta {
        item_id: Option<String>,
        delta: String,
    },
    /// Provider emitted the current turn plan snapshot.
    TurnPlanUpdated {
        explanation: Option<String>,
        plan: Vec<AgentPlanStep>,
    },
    /// Provider emitted the current turn diff snapshot.
    TurnDiffUpdated { diff: String },
    /// Provider emitted a raw response item completion payload.
    RawResponseItemCompleted { item: serde_json::Value },
    /// The model requested a tool invocation.
    ToolCallStart {
        /// Tool name as emitted by the model.
        name: String,
        /// Tool arguments as raw JSON.
        arguments: serde_json::Value,
    },
    /// A tool finished; `content` is the post-sanitization payload that
    /// is fed back into the next LLM iteration.
    ToolResult {
        /// Tool name as it appeared in the matching `ToolCallStart`.
        name: String,
        /// Sanitized tool output that the LLM will see next iteration.
        content: String,
        /// `true` when the executor returned an error tool_result or the
        /// egress gate replaced the content with a `[redacted: …]`
        /// placeholder.
        is_error: bool,
    },
    /// Stop reason for the LLM iteration that just ended.
    FinishReason(FinishReason),
    /// The configured approval policy flagged a tool call as needing
    /// human approval (issue #910, GUI blocker B4).
    ///
    /// The agent loop has paused waiting for the host to dispatch a
    /// matching approval decision for `request_id`. Until then no
    /// further [`AgentEvent`]s are emitted for this turn.
    ApprovalNeeded {
        /// Unique handle the GUI feeds back into the approval inbox.
        request_id: Uuid,
        /// Tool name as emitted by the model.
        tool_name: String,
        /// Raw tool arguments, mirroring the matching `ToolCallStart`
        /// payload.
        tool_arguments: serde_json::Value,
        /// Human-readable description supplied by the policy.
        description: String,
        /// Sanitised parameters preview the GUI should render — *not*
        /// the raw arguments.
        display_parameters: serde_json::Value,
        /// `true` when the GUI may surface an "approve always"
        /// affordance.
        allow_always: bool,
    },
    /// The agent run completed successfully.
    Completed(AgentRunOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentPlanStep {
    pub step: String,
    pub status: AgentPlanStepStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

/// Best-effort emit; a closed receiver is not a loop-fatal error.
async fn emit_event(tx: Option<&mpsc::Sender<AgentEvent>>, event: AgentEvent) {
    if let Some(tx) = tx {
        let _ = tx.send(event).await;
    }
}

/// Narrow LLM seam driven by [`run_agentic_loop`].
///
/// Adapters wrap a concrete LLM client (`dasclaw_llm_provider`, a mock,
/// a recorder, …) and translate from the provider-native response into a
/// [`RespondOutput`]. The loop calls [`Self::respond`] once per iteration
/// in non-streaming mode, and [`Self::respond_streaming`] when the host
/// wired up an event channel for token-level streaming.
///
/// The remaining methods replace the old `LoopDelegate` lifecycle hooks
/// (`check_signals` / `before_llm_call` / `handle_text_response` /
/// `on_tool_intent_nudge` / `after_iteration`). All have defaults so a
/// minimal "headless agent" responder only has to implement
/// [`Self::respond`].
///
/// # `Send + Sync` requirement
///
/// This trait requires `Send + Sync` because the loop accepts
/// `&dyn AgentResponder`. Responders using borrowed references (e.g.
/// scoped `ChatResponder<'a>`) must ensure all borrowed fields are
/// `Send + Sync`. This is load-bearing: a responder that needs to be
/// spawned into a detached task must use `Arc`-based ownership instead
/// of borrows.
#[async_trait]
pub trait AgentResponder: Send + Sync {
    /// Produce the next response for the given context.
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError>;

    /// Streaming-aware variant used when the host wired up an event
    /// channel (issue #908, GUI blocker B2).
    ///
    /// `event_tx` is the same channel the agent loop forwards
    /// [`AgentEvent`]s on. Implementations emit one or more
    /// [`AgentEvent::TextChunk`] events as the model produces user-visible
    /// text, optionally [`AgentEvent::ReasoningSummaryChunk`] for structured
    /// reasoning, and then return the final [`RespondOutput`] so the loop can
    /// dispatch to tool execution or finish.
    ///
    /// The default implementation calls [`Self::respond`] and forwards
    /// the final text (if any) as a single chunk, so every existing
    /// adapter stays wire-compatible without code changes. Adapters
    /// backed by streaming providers override this method to forward
    /// token-level deltas as they arrive.
    ///
    /// `ToolCallStart`, `ToolResult` and `FinishReason` events are emitted
    /// by the agent loop itself — implementations should only emit text or
    /// reasoning chunks.
    async fn respond_streaming(
        &self,
        ctx: &mut ReasoningContext,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RespondOutput, HostError> {
        let out = self.respond(ctx).await?;
        if let RespondResult::Text(ref text) = out.result {
            // Drop on closed channel is fine: the consumer hung up, but
            // the loop still needs to return the full RespondOutput.
            let _ = event_tx.send(AgentEvent::TextChunk(text.clone())).await;
        }
        Ok(out)
    }

    /// Respond according to the explicit model-call policy.
    ///
    /// The default keeps generic responders source-compatible: invoke
    /// mode calls [`Self::respond`], while stream mode calls
    /// [`Self::respond_streaming`] when an event channel exists. Provider
    /// adapters override this method so native stream support is enforced
    /// by [`ModelCallMode`] instead of inferred from the caller's event
    /// channel.
    async fn respond_with_policy(
        &self,
        ctx: &mut ReasoningContext,
        policy: AgentCallPolicy,
    ) -> Result<RespondOutput, HostError> {
        match (policy.model_call_mode, policy.event_tx) {
            (ModelCallMode::Invoke, Some(event_tx)) => {
                let out = self.respond(ctx).await?;
                if let RespondResult::Text(ref text) = out.result {
                    let _ = event_tx.send(AgentEvent::TextChunk(text.clone())).await;
                }
                Ok(out)
            }
            (ModelCallMode::Invoke, None) => self.respond(ctx).await,
            (ModelCallMode::Stream, Some(event_tx)) => self.respond_streaming(ctx, event_tx).await,
            (ModelCallMode::Stream, None) => self.respond(ctx).await,
        }
    }

    /// Per-iteration signal check.
    ///
    /// Returned every loop turn before any LLM call. The default keeps
    /// the loop running; hosts that drain a stop / cancellation channel
    /// override this. The loop's own cancellation token (passed to
    /// [`run_agentic_loop`]) is honoured independently — a `Continue`
    /// here cannot override an already-cancelled token.
    async fn check_signals(&self) -> LoopSignal {
        LoopSignal::Continue
    }

    /// Pre-LLM iteration setup.
    ///
    /// Runs after [`Self::check_signals`] and before [`Self::respond`].
    /// Hosts use it to mutate the context (inject prompts, swap tool
    /// tables, force text mode) or short-circuit the loop by returning
    /// `Some(outcome)`. The default is a no-op.
    async fn before_llm_call(
        &self,
        _ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Option<LoopOutcome> {
        None
    }

    /// React to a pure-text LLM response.
    ///
    /// Called when [`Self::respond`] yields a [`RespondResult::Text`]
    /// payload. The default appends the assistant message to `ctx` and
    /// terminates the loop with [`LoopOutcome::Response`], matching the
    /// pre-W10 headless-agent behaviour. Hosts that need recovery /
    /// completion-detection logic override this and may return
    /// [`TextAction::Continue`] to keep iterating.
    async fn handle_text_response(
        &self,
        text: &str,
        _metadata: ResponseMetadata,
        usage: TokenUsage,
        ctx: &mut ReasoningContext,
    ) -> TextAction {
        ctx.messages
            .push(ChatMessage::assistant(text).with_usage(usage));
        TextAction::Return(LoopOutcome::Response(text.to_string()))
    }

    /// React to a tool-intent nudge being injected.
    ///
    /// Called when the loop detects the model produced text that looked
    /// like a tool intent and injects a corrective nudge. Default is a
    /// no-op; hosts override to emit a UI event.
    async fn on_tool_intent_nudge(&self, _text: &str, _ctx: &mut ReasoningContext) {}

    /// End-of-iteration callback.
    ///
    /// Called after every successful iteration that did not return an
    /// outcome. Default is a no-op; hosts use it for throttling /
    /// progress reporting.
    async fn after_iteration(&self, _iteration: usize) {}
}

/// Narrow tool-execution seam driven by [`run_agentic_loop`].
///
/// Wraps a concrete tool-dispatch pipeline (ADR-153 §1.1 5-step pipeline,
/// a parallel dispatcher, a replay engine, a mock, …) and drives a
/// single iteration's worth of tool calls. The loop calls
/// [`Self::dispatch`] once per iteration that produced tool calls;
/// dispatchers that need to record the assistant tool-call turn into
/// the context must do so themselves before executing.
///
/// When no dispatcher is wired and the model still emits a tool call,
/// the loop ends with [`LoopOutcome::Failure`] carrying
/// [`TOOLS_NOT_SUPPORTED_REASON`].
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

/// Run the unified agentic loop.
///
/// `responder` provides LLM I/O and per-iteration lifecycle hooks.
/// `dispatcher` is optional: when `None`, tool calls from the model
/// surface as [`LoopOutcome::Failure`] with [`TOOLS_NOT_SUPPORTED_REASON`].
/// `cancellation_token` and `event_tx` are also optional — passing both
/// `None` yields the minimum "headless" loop.
///
/// `hooks` is the environment bundle ([`HookBundle`]). The loop itself
/// calls two of the four hooks directly:
///
/// - `egress.check(EgressKind::LlmRequest, …)` on the most recent user
///   message before each LLM call. A `Block` decision ends the loop with
///   `LoopOutcome::Failure`. A `Redact` swaps the message content with
///   the sanitized payload.
/// - `egress.check(EgressKind::UserDisplay, …)` on text-only LLM responses
///   before the responder sees them. `Redact` swaps the text in place so
///   downstream UI sees the sanitized version.
///
/// Tool-level egress (`EgressKind::ToolExecution`) and approval gating
/// are the responsibility of the [`ToolDispatcher`] implementation.
/// Dispatchers typically clone the same `Arc<HookBundle>` at
/// construction time.
#[allow(clippy::too_many_arguments)]
pub async fn run_agentic_loop(
    responder: &dyn AgentResponder,
    dispatcher: Option<&dyn ToolDispatcher>,
    cancellation_token: Option<&CancellationToken>,
    model_call_mode: ModelCallMode,
    event_tx: Option<&mpsc::Sender<AgentEvent>>,
    reason_ctx: &mut ReasoningContext,
    config: &AgenticLoopConfig,
    hooks: &HookBundle,
) -> Result<LoopOutcome, HostError> {
    let mut consecutive_tool_intent_nudges: u32 = 0;
    // Accumulates across all iterations (not reset by text responses) so
    // non-consecutive truncations still escalate to force_text.
    let mut truncation_count: u32 = 0;

    for iteration in 1..=config.max_iterations {
        // Check for external signals (stop, cancellation, user messages).
        // A cancelled token always wins even if the responder would
        // return `Continue`.
        let signal = if cancellation_token.is_some_and(CancellationToken::is_cancelled) {
            LoopSignal::Stop
        } else {
            responder.check_signals().await
        };
        match signal {
            LoopSignal::Continue => {}
            LoopSignal::Stop => return Ok(LoopOutcome::Stopped),
            LoopSignal::InjectMessage(msg) => {
                reason_ctx.messages.push(ChatMessage::user(&msg));
            }
        }

        // Pre-LLM call hook (cost guard, tool refresh, iteration limit
        // nudge)
        if let Some(outcome) = responder.before_llm_call(reason_ctx, iteration).await {
            return Ok(outcome);
        }

        // EgressGate (ADR-148 Layer B): scan / redact the most recent user
        // message before the LLM sees it. A Redact decision swaps the
        // payload with the sanitized version so the LLM call uses it.
        // The five-arm decision handling lives in `apply_egress_decision`
        // so every gate site in the workspace halts identically on
        // Block / Ask / unknown variant.
        if let Some(idx) = reason_ctx
            .messages
            .iter()
            .rposition(|m| m.role == Role::User)
        {
            let prompt = &mut reason_ctx.messages[idx].content;
            let decision = hooks.egress.check(&EgressKind::LlmRequest, prompt).await;
            if let EgressApply::Halt(reason) = apply_egress_decision(decision, prompt, "LlmRequest")
            {
                tracing::warn!(iteration, %reason, "egress gate halted LlmRequest");
                return Ok(LoopOutcome::Failure(reason));
            }
        }

        // Call LLM (streaming when an event channel is wired, plain
        // otherwise). Forward the trailing `FinishReason` so a GUI knows
        // the iteration boundary even when the model emitted no text.
        let mut output = responder
            .respond_with_policy(
                reason_ctx,
                AgentCallPolicy {
                    model_call_mode,
                    event_tx: event_tx.cloned(),
                },
            )
            .await?;
        emit_event(event_tx, AgentEvent::FinishReason(output.finish_reason)).await;

        // EgressGate (ADR-148 Layer B): scan / redact text completions
        // before the responder (and ultimately the user UI) sees them.
        // Tool-call responses skip this gate; dispatchers apply the
        // tool-level egress themselves inside `dispatch`.
        if let RespondResult::Text(ref mut text) = output.result {
            let decision = hooks.egress.check(&EgressKind::UserDisplay, text).await;
            if let EgressApply::Halt(reason) = apply_egress_decision(decision, text, "UserDisplay")
            {
                tracing::warn!(iteration, %reason, "egress gate halted UserDisplay");
                return Ok(LoopOutcome::Failure(reason));
            }
        }

        match &output.result {
            RespondResult::Text(text) => {
                tracing::debug!(
                    iteration,
                    len = text.len(),
                    has_suggestions = text.contains("<suggestions>"),
                    response = %text,
                    "LLM text response"
                );
            }
            RespondResult::ToolCalls {
                tool_calls,
                content,
            } => {
                let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                tracing::debug!(
                    iteration,
                    tools = ?names,
                    has_content = content.is_some(),
                    "LLM tool_calls response"
                );
            }
        }

        match output.result {
            RespondResult::Text(text) => {
                let usage = output.usage;
                // Tool intent nudge: if the LLM says "let me search..."
                // without actually calling a tool, inject a nudge message.
                if config.enable_tool_intent_nudge
                    && !reason_ctx.available_tools.is_empty()
                    && !reason_ctx.force_text
                    && consecutive_tool_intent_nudges < config.max_tool_intent_nudges
                    && llm_signals_tool_intent(&text)
                {
                    consecutive_tool_intent_nudges += 1;
                    tracing::info!(
                        iteration,
                        "LLM expressed tool intent without calling a tool, nudging"
                    );
                    responder.on_tool_intent_nudge(&text, reason_ctx).await;
                    reason_ctx
                        .messages
                        .push(ChatMessage::assistant(&text).with_usage(usage));
                    reason_ctx
                        .messages
                        .push(ChatMessage::user(TOOL_INTENT_NUDGE));
                    responder.after_iteration(iteration).await;
                    continue;
                }

                // Reset nudge counter since we got a non-intent text
                // response.
                if !llm_signals_tool_intent(&text) {
                    consecutive_tool_intent_nudges = 0;
                }

                match responder
                    .handle_text_response(&text, output.metadata, usage, reason_ctx)
                    .await
                {
                    TextAction::Return(outcome) => return Ok(outcome),
                    TextAction::Continue => {}
                }
            }
            RespondResult::ToolCalls {
                tool_calls,
                content,
            } => {
                let usage = output.usage;
                // If the response was truncated, tool call parameters are
                // likely incomplete. Discard them and tell the LLM to try a
                // different approach rather than executing malformed calls.
                if output.finish_reason == FinishReason::Length {
                    truncation_count += 1;
                    let names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
                    tracing::warn!(
                        iteration,
                        tools = ?names,
                        truncation_count,
                        "Discarding truncated tool calls (finish_reason=Length)"
                    );
                    if let Some(ref text) = content {
                        reason_ctx
                            .messages
                            .push(ChatMessage::assistant(text).with_usage(usage));
                    }
                    reason_ctx
                        .messages
                        .push(ChatMessage::user(TRUNCATED_TOOL_CALL_NOTICE));
                    // After repeated truncations, force text-only mode so
                    // the LLM stops attempting tool calls it can't fit in
                    // the output budget.
                    if truncation_count >= 3 {
                        reason_ctx.force_text = true;
                    }
                    responder.after_iteration(iteration).await;
                    continue;
                }

                consecutive_tool_intent_nudges = 0;
                truncation_count = 0;

                // No dispatcher wired → emit the sentinel and let the
                // host map it back to `AgentError::ToolsNotSupported`.
                let outcome_opt = match dispatcher {
                    Some(d) => d.dispatch(tool_calls, content, usage, reason_ctx).await?,
                    None => Some(LoopOutcome::Failure(TOOLS_NOT_SUPPORTED_REASON.to_string())),
                };
                if let Some(outcome) = outcome_opt {
                    return Ok(outcome);
                }
            }
        }

        responder.after_iteration(iteration).await;
    }

    Ok(LoopOutcome::MaxIterations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{Role, ToolCall, ToolDefinition};
    use crate::response_types::{ResponseAnomaly, TokenUsage};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    fn zero_usage() -> TokenUsage {
        TokenUsage::default()
    }

    fn text_output(text: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::Text(text.to_string()),
            usage: zero_usage(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        }
    }

    fn tool_calls_output(calls: Vec<ToolCall>) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: calls,
                content: None,
            },
            usage: zero_usage(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    /// Default invocation: run with no dispatcher / cancellation / event
    /// channel, matching the legacy `run_agentic_loop(&delegate, …)`
    /// shape used by the pre-W10.1 tests.
    async fn run_with(
        responder: &dyn AgentResponder,
        ctx: &mut ReasoningContext,
        config: &AgenticLoopConfig,
        hooks: &HookBundle,
    ) -> Result<LoopOutcome, HostError> {
        run_agentic_loop(
            responder,
            None,
            None,
            ModelCallMode::Invoke,
            None,
            ctx,
            config,
            hooks,
        )
        .await
    }

    /// Configurable mock responder + dispatcher pair for driving
    /// `run_agentic_loop`. Splits the old `MockDelegate` cleanly along the
    /// new trait boundary.
    struct MockResponder {
        signal: Mutex<LoopSignal>,
        llm_responses: Mutex<Vec<RespondOutput>>,
        iterations_seen: Mutex<Vec<usize>>,
        early_exit: Mutex<Option<(usize, LoopOutcome)>>,
        nudge_count: AtomicUsize,
    }

    impl MockResponder {
        fn new(responses: Vec<RespondOutput>) -> Self {
            Self {
                signal: Mutex::new(LoopSignal::Continue),
                llm_responses: Mutex::new(responses),
                iterations_seen: Mutex::new(Vec::new()),
                early_exit: Mutex::new(None),
                nudge_count: AtomicUsize::new(0),
            }
        }

        fn with_signal(mut self, signal: LoopSignal) -> Self {
            self.signal = Mutex::new(signal);
            self
        }

        fn with_early_exit(mut self, iteration: usize, outcome: LoopOutcome) -> Self {
            self.early_exit = Mutex::new(Some((iteration, outcome)));
            self
        }
    }

    #[async_trait]
    impl AgentResponder for MockResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            let mut responses = self.llm_responses.lock().await;
            assert!(
                !responses.is_empty(),
                "MockResponder: no more LLM responses queued"
            );
            Ok(responses.remove(0))
        }

        async fn check_signals(&self) -> LoopSignal {
            let mut sig = self.signal.lock().await;
            std::mem::replace(&mut *sig, LoopSignal::Continue)
        }

        async fn before_llm_call(
            &self,
            _ctx: &mut ReasoningContext,
            iteration: usize,
        ) -> Option<LoopOutcome> {
            let mut guard = self.early_exit.lock().await;
            let should_take = guard
                .as_ref()
                .is_some_and(|(target, _)| *target == iteration);
            if should_take {
                guard.take().map(|(_, o)| o)
            } else {
                None
            }
        }

        async fn handle_text_response(
            &self,
            text: &str,
            _metadata: ResponseMetadata,
            _usage: TokenUsage,
            _ctx: &mut ReasoningContext,
        ) -> TextAction {
            // Override the default (which appends + Returns) so the
            // assertion shape of the pre-W10 tests carries over unchanged.
            TextAction::Return(LoopOutcome::Response(text.to_string()))
        }

        async fn on_tool_intent_nudge(&self, _text: &str, _ctx: &mut ReasoningContext) {
            self.nudge_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn after_iteration(&self, iteration: usize) {
            self.iterations_seen.lock().await.push(iteration);
        }
    }

    struct MockDispatcher {
        tool_exec_count: AtomicUsize,
        tool_exec_outcome: Mutex<Option<LoopOutcome>>,
    }

    impl MockDispatcher {
        fn new() -> Self {
            Self {
                tool_exec_count: AtomicUsize::new(0),
                tool_exec_outcome: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ToolDispatcher for MockDispatcher {
        async fn dispatch(
            &self,
            _tool_calls: Vec<ToolCall>,
            _content: Option<String>,
            _usage: TokenUsage,
            ctx: &mut ReasoningContext,
        ) -> Result<Option<LoopOutcome>, HostError> {
            self.tool_exec_count.fetch_add(1, Ordering::SeqCst);
            ctx.messages.push(ChatMessage::user("tool result stub"));
            Ok(self.tool_exec_outcome.lock().await.take())
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_text_response_returns_immediately() {
        let responder = MockResponder::new(vec![text_output("Hello, world!")]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        match outcome {
            LoopOutcome::Response(text) => assert_eq!(text, "Hello, world!"),
            other => panic!("expected LoopOutcome::Response, got {other:?}-ish"),
        }
        assert!(responder.iterations_seen.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_tool_call_then_text_response() {
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({}),
            reasoning: None,
        };
        let responder = MockResponder::new(vec![
            tool_calls_output(vec![tool_call]),
            text_output("Done!"),
        ]);
        let dispatcher = MockDispatcher::new();
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(
            &responder,
            Some(&dispatcher),
            None,
            ModelCallMode::Invoke,
            None,
            &mut ctx,
            &config,
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        match outcome {
            LoopOutcome::Response(text) => assert_eq!(text, "Done!"),
            other => panic!("expected LoopOutcome::Response, got {other:?}-ish"),
        }
        assert_eq!(dispatcher.tool_exec_count.load(Ordering::SeqCst), 1);
        assert_eq!(*responder.iterations_seen.lock().await, vec![1]);
    }

    #[tokio::test]
    async fn test_tool_call_without_dispatcher_yields_tools_not_supported() {
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({}),
            reasoning: None,
        };
        let responder = MockResponder::new(vec![tool_calls_output(vec![tool_call])]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        match outcome {
            LoopOutcome::Failure(reason) => assert_eq!(reason, TOOLS_NOT_SUPPORTED_REASON),
            other => panic!("expected Failure(tools_not_supported), got {other:?}-ish"),
        }
    }

    #[tokio::test]
    async fn test_stop_signal_exits_immediately() {
        let responder =
            MockResponder::new(vec![text_output("unreachable")]).with_signal(LoopSignal::Stop);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Stopped));
        assert!(responder.iterations_seen.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_cancellation_token_short_circuits() {
        let responder = MockResponder::new(vec![text_output("unreachable")]);
        let token = CancellationToken::new();
        token.cancel();
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(
            &responder,
            None,
            Some(&token),
            ModelCallMode::Invoke,
            None,
            &mut ctx,
            &config,
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Stopped));
    }

    #[tokio::test]
    async fn test_inject_message_adds_user_message() {
        let responder = MockResponder::new(vec![text_output("Got it")])
            .with_signal(LoopSignal::InjectMessage("injected prompt".to_string()));
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Response(_)));
        assert!(
            ctx.messages
                .iter()
                .any(|m| m.role == Role::User && m.content.contains("injected prompt")),
            "Injected message should appear in context"
        );
    }

    #[tokio::test]
    async fn test_text_response_metadata_can_fail_fast() {
        struct FailOnMalformedResponse;

        #[async_trait]
        impl AgentResponder for FailOnMalformedResponse {
            async fn respond(&self, _: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
                Ok(RespondOutput {
                    result: RespondResult::Text("fallback".to_string()),
                    usage: zero_usage(),
                    finish_reason: FinishReason::Stop,
                    metadata: ResponseMetadata {
                        anomaly: Some(ResponseAnomaly::EmptyToolCompletion),
                    },
                })
            }

            async fn handle_text_response(
                &self,
                _: &str,
                metadata: ResponseMetadata,
                _: TokenUsage,
                _: &mut ReasoningContext,
            ) -> TextAction {
                assert_eq!(metadata.anomaly, Some(ResponseAnomaly::EmptyToolCompletion));
                TextAction::Return(LoopOutcome::Failure(
                    "malformed tool completion".to_string(),
                ))
            }
        }

        let responder = FailOnMalformedResponse;
        let mut ctx = ReasoningContext::new();
        let outcome = run_with(
            &responder,
            &mut ctx,
            &AgenticLoopConfig::default(),
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        assert!(
            matches!(outcome, LoopOutcome::Failure(ref reason) if reason == "malformed tool completion")
        );
    }

    #[tokio::test]
    async fn test_max_iterations_reached() {
        struct ContinueResponder;

        #[async_trait]
        impl AgentResponder for ContinueResponder {
            async fn respond(&self, _: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
                Ok(text_output("still working"))
            }
            async fn handle_text_response(
                &self,
                _: &str,
                _: ResponseMetadata,
                _: TokenUsage,
                ctx: &mut ReasoningContext,
            ) -> TextAction {
                ctx.messages.push(ChatMessage::assistant("still working"));
                TextAction::Continue
            }
        }

        let responder = ContinueResponder;
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 3,
            ..Default::default()
        };

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::MaxIterations));
        let assistant_count = ctx
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .count();
        assert_eq!(assistant_count, 3);
    }

    #[tokio::test]
    async fn test_tool_intent_nudge_fires_and_caps() {
        let responder = MockResponder::new(vec![
            text_output("Let me search for that file"),
            text_output("Let me search for that file"),
            text_output("Let me search for that file"),
        ]);
        let mut ctx = ReasoningContext::new();
        ctx.available_tools.push(ToolDefinition {
            name: "search".to_string(),
            description: "Search files".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        });
        let config = AgenticLoopConfig {
            max_iterations: 10,
            enable_tool_intent_nudge: true,
            max_tool_intent_nudges: 2,
        };

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Response(_)));
        assert_eq!(responder.nudge_count.load(Ordering::SeqCst), 2);
        let nudge_messages = ctx
            .messages
            .iter()
            .filter(|m| {
                m.role == Role::User && m.content.contains("you did not include any tool calls")
            })
            .count();
        assert_eq!(
            nudge_messages, 2,
            "Should have exactly 2 nudge messages in context"
        );
    }

    #[tokio::test]
    async fn test_before_llm_call_early_exit() {
        let responder = MockResponder::new(vec![text_output("unreachable")])
            .with_early_exit(1, LoopOutcome::Stopped);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_with(&responder, &mut ctx, &config, &HookBundle::noop())
            .await
            .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Stopped));
        assert!(responder.iterations_seen.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_truncated_tool_calls_discarded_on_length() {
        let truncated_tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "memory_write".to_string(),
            arguments: serde_json::json!({}),
            reasoning: None,
        };
        let truncated_output = RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![truncated_tool_call],
                content: Some("I'll write the report.".to_string()),
            },
            usage: zero_usage(),
            finish_reason: FinishReason::Length,
            metadata: ResponseMetadata::default(),
        };
        let responder = MockResponder::new(vec![truncated_output, text_output("Summarized it.")]);
        let dispatcher = MockDispatcher::new();
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let outcome = run_agentic_loop(
            &responder,
            Some(&dispatcher),
            None,
            ModelCallMode::Invoke,
            None,
            &mut ctx,
            &config,
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        assert_eq!(dispatcher.tool_exec_count.load(Ordering::SeqCst), 0);
        assert!(matches!(outcome, LoopOutcome::Response(ref t) if t == "Summarized it."));
        assert!(
            ctx.messages
                .iter()
                .any(|m| m.role == Role::User && m.content.contains("truncated")),
            "Should inject truncation notice into context"
        );
        assert!(
            ctx.messages
                .iter()
                .any(|m| m.role == Role::Assistant && m.content.contains("write the report")),
            "Should preserve partial assistant content"
        );
    }

    #[tokio::test]
    async fn test_repeated_truncations_force_text_mode() {
        let make_truncated = || RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "memory_write".to_string(),
                    arguments: serde_json::json!({}),
                    reasoning: None,
                }],
                content: None,
            },
            usage: zero_usage(),
            finish_reason: FinishReason::Length,
            metadata: ResponseMetadata::default(),
        };
        let responder = MockResponder::new(vec![
            make_truncated(),
            make_truncated(),
            make_truncated(),
            text_output("Gave up on tool calls."),
        ]);
        let dispatcher = MockDispatcher::new();
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let outcome = run_agentic_loop(
            &responder,
            Some(&dispatcher),
            None,
            ModelCallMode::Invoke,
            None,
            &mut ctx,
            &config,
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        assert!(matches!(outcome, LoopOutcome::Response(_)));
        assert_eq!(dispatcher.tool_exec_count.load(Ordering::SeqCst), 0);
        assert!(
            ctx.force_text,
            "Should escalate to force_text after repeated truncations"
        );
    }

    // -----------------------------------------------------------------
    // EgressGate contract tests (ADR-148; replaces former SafetyHook tests)
    // -----------------------------------------------------------------

    use crate::hooks::{EgressDecision, EgressGate, EgressKind, RedactionStats};

    /// Gate that blocks every LlmRequest with a fixed reason.
    struct BlockAllPrompts;

    #[async_trait]
    impl EgressGate for BlockAllPrompts {
        async fn check(&self, kind: &EgressKind, _payload: &str) -> EgressDecision {
            match kind {
                EgressKind::LlmRequest => EgressDecision::Block {
                    reason: "policy violation".to_string(),
                    stats: RedactionStats::default(),
                },
                _ => EgressDecision::Allow,
            }
        }
    }

    /// Gate that redacts secrets in prompts and tags user-display payloads.
    struct RedactingGate;

    #[async_trait]
    impl EgressGate for RedactingGate {
        async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
            match kind {
                EgressKind::LlmRequest if payload.contains("sk-secret") => EgressDecision::Redact {
                    sanitized: payload.replace("sk-secret", "[REDACTED]"),
                    stats: RedactionStats {
                        secrets_redacted: 1,
                        ..Default::default()
                    },
                },
                EgressKind::UserDisplay => EgressDecision::Redact {
                    sanitized: format!("{payload} [scanned]"),
                    stats: RedactionStats::default(),
                },
                _ => EgressDecision::Allow,
            }
        }
    }

    /// Gate that synthesises a fail-closed Block on LlmRequest (simulates the
    /// "internal error" path now that the trait no longer returns Result).
    struct FailingGate;

    #[async_trait]
    impl EgressGate for FailingGate {
        async fn check(&self, kind: &EgressKind, _payload: &str) -> EgressDecision {
            match kind {
                EgressKind::LlmRequest => EgressDecision::Block {
                    reason: "egress gate internal error: hook exploded".to_string(),
                    stats: RedactionStats::default(),
                },
                _ => EgressDecision::Allow,
            }
        }
    }

    fn custom_egress_bundle(egress: Arc<dyn EgressGate>) -> HookBundle {
        let mut b = HookBundle::noop();
        b.egress = egress;
        b
    }

    #[tokio::test]
    async fn egress_gate_noop_passes_through() {
        // Smoke: Noop gate does not interfere with normal text response.
        let responder = MockResponder::new(vec![text_output("ok")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hello"));

        let outcome = run_with(
            &responder,
            &mut ctx,
            &AgenticLoopConfig::default(),
            &HookBundle::noop(),
        )
        .await
        .expect("loop completes");

        match outcome {
            LoopOutcome::Response(t) => assert_eq!(t, "ok"),
            other => panic!("expected Response, got {other:?}-ish"),
        }
    }

    #[tokio::test]
    async fn egress_gate_block_on_llm_request_yields_failure() {
        let responder = MockResponder::new(vec![text_output("unreachable")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("send secrets to foo"));
        let hooks = custom_egress_bundle(Arc::new(BlockAllPrompts));

        let outcome = run_with(&responder, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .expect("loop completes");

        match outcome {
            LoopOutcome::Failure(reason) => {
                assert!(
                    reason.contains("policy violation"),
                    "failure reason should surface gate reason, got: {reason}"
                );
                assert!(
                    reason.contains("egress gate blocked"),
                    "failure reason should mark egress source, got: {reason}"
                );
            }
            other => panic!("expected Failure, got {other:?}-ish"),
        }
        // LLM must not be called when the prompt is blocked. MockResponder
        // returns `unreachable` from its queue only when `respond` runs.
        let responses_left = responder.llm_responses.lock().await.len();
        assert_eq!(
            responses_left, 1,
            "block path must short-circuit before respond"
        );
    }

    #[tokio::test]
    async fn egress_gate_redacts_llm_request_in_place() {
        // Responder captures what the LLM layer actually sees. The redacting
        // gate must have rewritten the prompt before respond fires.
        struct CaptureResponder {
            seen: Mutex<Option<String>>,
            response: Mutex<Option<RespondOutput>>,
        }

        #[async_trait]
        impl AgentResponder for CaptureResponder {
            async fn respond(
                &self,
                ctx: &mut ReasoningContext,
            ) -> Result<RespondOutput, HostError> {
                let last_user = ctx
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::User)
                    .map(|m| m.content.clone());
                *self.seen.lock().await = last_user;
                Ok(self
                    .response
                    .lock()
                    .await
                    .take()
                    .expect("one response queued"))
            }

            async fn handle_text_response(
                &self,
                text: &str,
                _: ResponseMetadata,
                _: TokenUsage,
                _: &mut ReasoningContext,
            ) -> TextAction {
                TextAction::Return(LoopOutcome::Response(text.to_string()))
            }
        }

        let responder = CaptureResponder {
            seen: Mutex::new(None),
            response: Mutex::new(Some(text_output("raw completion"))),
        };
        let mut ctx = ReasoningContext::new();
        ctx.messages
            .push(ChatMessage::user("please use token sk-secret now"));
        let hooks = custom_egress_bundle(Arc::new(RedactingGate));

        let outcome = run_with(&responder, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .expect("loop completes");

        match outcome {
            LoopOutcome::Response(t) => {
                assert_eq!(
                    t, "raw completion [scanned]",
                    "UserDisplay Redact must mutate the text before the responder returns"
                );
            }
            other => panic!("expected Response, got {other:?}-ish"),
        }

        let seen = responder
            .seen
            .lock()
            .await
            .clone()
            .expect("respond saw a user message");
        assert!(
            !seen.contains("sk-secret"),
            "LlmRequest Redact must rewrite the payload; LLM saw: {seen}"
        );
        assert!(
            seen.contains("[REDACTED]"),
            "redaction token must be present; LLM saw: {seen}"
        );

        let stored = ctx
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .expect("ctx still has the user message")
            .content
            .clone();
        assert!(
            !stored.contains("sk-secret"),
            "ctx must retain redacted form, got: {stored}"
        );
    }

    #[tokio::test]
    async fn egress_gate_internal_error_translates_to_failure() {
        // ADR-148: fail-closed contract — internal errors surface as Block,
        // not panics or HostError. The loop converts Block into Failure.
        let responder = MockResponder::new(vec![text_output("unreachable")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("anything"));
        let hooks = custom_egress_bundle(Arc::new(FailingGate));

        let outcome = run_with(&responder, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .expect("FailingGate must NOT bubble HostError — fail-closed translates to Failure");

        match outcome {
            LoopOutcome::Failure(reason) => {
                assert!(
                    reason.contains("hook exploded"),
                    "failure reason should preserve inner cause, got: {reason}"
                );
            }
            other => panic!("expected Failure, got {other:?}-ish"),
        }
    }
}
