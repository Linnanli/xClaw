//! Headless agent facade (ADR-153 step 2, sub-step A1).
//!
//! Goal: let downstream code spin up an agent in four lines, without
//! pulling in Tauri, sessions, channels, databases or any of the
//! desktop infrastructure:
//!
//! ```ignore
//! use dasclaw_runtime::{Agent, AgentResponder};
//!
//! let agent = Agent::builder()
//!     .responder(my_responder)        // anything that knows how to call an LLM
//!     .system_prompt("You are a helpful assistant.")
//!     .build()?;
//! let text = agent.run("Hello!").await?;
//! ```
//!
//! ## Scope (A1 + A2)
//!
//! - **A1** — public shape (`Agent`, `AgentBuilder`, `AgentConfig`,
//!   `AgentError`), wraps the [`AgenticLoop`] runtime engine with a
//!   configurable [`HookBundle`]. The narrow [`AgentResponder`] trait
//!   (re-exported from [`dasclaw_core::agentic_loop`]) is the single
//!   seam where any LLM adapter plugs in.
//! - **A2** — tool execution wiring. A second narrow trait
//!   [`ToolExecutor`] lets callers plug in a real tool runner. When wired,
//!   the loop dispatches each tool call through the executor and feeds the
//!   results back into [`ReasoningContext`] as assistant + tool_result
//!   messages. When no executor is wired and the model emits tool calls,
//!   the loop still ends with [`AgentError::ToolsNotSupported`].
//! - **A2** — ships [`llm_adapter::LlmProviderResponder`] in a sibling
//!   module, adapting any [`dasclaw_llm_provider::provider::LlmProvider`]
//!   into an [`AgentResponder`].
//!
//! ## Why a new `AgentResponder` trait instead of `LlmCompleter`
//!
//! [`dasclaw_core::traits::LlmCompleter`] is a **text-only** facade
//! intended for compaction summaries and similar one-shot helpers — it
//! returns a `String`, not a `RespondOutput`, so it cannot carry tool
//! calls or finish reasons. The agent loop fundamentally needs
//! `RespondOutput`, so the [`AgentResponder`] seam is a dedicated
//! single-method trait — adapter authors only ever need to implement
//! `respond` (and optionally `respond_streaming` for token streaming).

use std::sync::Arc;

use async_trait::async_trait;
pub(crate) use dasclaw_core::agentic_loop::TOOLS_NOT_SUPPORTED_REASON;
pub use dasclaw_core::agentic_loop::{AgentEvent, AgentResponder};
use dasclaw_core::agentic_loop::{AgenticLoopConfig, LoopOutcome};
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ChatMessage, ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::traits::HostError;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agentic_loop::AgenticLoop;
use crate::approval::{
    ApprovalDecision, ApprovalDispatchError, ApprovalInbox, ApprovalPolicy, NoApprovalPolicy,
    PolicyApprover,
};
use crate::tool_dispatch::{
    APPROVAL_REJECTED_SENTINEL_PREFIX, RejectedPayload, SequentialDispatcher, ToolDispatcher,
};

/// Narrow tool-execution seam used by [`Agent`] (ADR-153 step 2 sub-step A2).
///
/// Wraps a concrete tool runner — a `dasclaw_tool` registry, an MCP
/// gateway, a recorder, or a mock — and runs a single tool call to
/// completion. The runtime calls `execute` once per tool call emitted by
/// the model, sequentially, and stores the resulting [`ToolResult`] in
/// the conversation as the canonical assistant + tool_result pair before
/// looping back into the next LLM call.
///
/// When no executor is plugged in and the model still emits a tool call,
/// the agent ends the loop with [`AgentError::ToolsNotSupported`].
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Run a single tool call and return its result.
    ///
    /// Implementations are responsible for their own approval, egress and
    /// sandbox enforcement. The runtime does not retry on error — return
    /// a `ToolResult` with `is_error = true` to feed the failure back to
    /// the model.
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError>;
}

/// Forward [`ToolExecutor`] through an `Arc<dyn ToolExecutor>` so callers
/// that erase the concrete type (CLI dispatch, plugin registries,
/// dynamically-assembled tool stacks) can still feed the agent loop.
///
/// Without this blanket impl, [`crate::Agent`]'s generic
/// `E: ToolExecutor + 'static` bound rejects `Arc<dyn ToolExecutor>`
/// even though the trait object itself satisfies the trait via dynamic
/// dispatch — Rust does not auto-implement traits for `Arc<dyn Trait>`.
#[async_trait]
impl ToolExecutor for std::sync::Arc<dyn ToolExecutor> {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        (**self).execute(call).await
    }
}

/// Narrow tool-output sanitization seam used by [`Agent`]
/// (ADR-153 §1.1 B6 / e22 wiring).
///
/// Sits between [`ToolExecutor::execute`] and the message-history push,
/// so the redacted content is what the **next** LLM iteration sees in
/// its `tool_result` block. Implementations typically delegate to a
/// safety crate (e.g. `dasclaw_safety::SafetyLayer::sanitize_for_stash`),
/// but the runtime stays safety-stack-agnostic: any `Fn`-like wrapper
/// works as long as it returns the post-redaction string. The production
/// binding is provided by `dasclaw_cli::run_with_tools_and_safety_sanitizer`
/// (issue #882), which installs the safety-layer-backed adapter as the
/// default for the CLI agent loop.
///
/// When no sanitizer is wired (the default), tool output is forwarded
/// to the LLM **verbatim**. The trait is sync because production
/// sanitizers run regex passes that don't await, and forcing async would
/// push spurious `Box::pin` allocations into the hot loop path.
pub trait ToolOutputSanitizer: Send + Sync {
    /// Return the sanitized content for `tool_name`'s output. The
    /// implementation owns any redaction, masking, policy enforcement
    /// and length capping; the runtime treats the return value as
    /// authoritative and pushes it straight into the conversation.
    fn sanitize(&self, tool_name: &str, content: &str) -> String;
}

/// Lifecycle stage emitted around a real [`ToolExecutor::execute`] call.
///
/// This is intentionally runtime-level production API, not a test helper:
/// hosts can attach tracing, audit, or compliance observers to the exact
/// executor boundary the agent loop uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolLifecycleStage {
    /// Immediately before the executor is invoked, after approval and
    /// pre-execute egress have allowed the call.
    PreToolUse,
    /// Immediately after the executor returns a [`ToolResult`], before
    /// post-execute egress and output sanitization rewrite the payload.
    PostToolUse,
}

/// Metadata for one tool lifecycle callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolLifecycleEvent {
    pub stage: ToolLifecycleStage,
    pub tool_call_id: String,
    pub tool_name: String,
}

/// Observer for production tool lifecycle callbacks.
#[async_trait]
pub trait ToolLifecycleObserver: Send + Sync {
    /// Called by the runtime around real tool execution.
    async fn observe(&self, event: ToolLifecycleEvent) -> Result<(), HostError>;
}

/// Default observer used when hosts do not need lifecycle callbacks.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopToolLifecycleObserver;

#[async_trait]
impl ToolLifecycleObserver for NoopToolLifecycleObserver {
    async fn observe(&self, _event: ToolLifecycleEvent) -> Result<(), HostError> {
        Ok(())
    }
}

/// Errors surfaced by [`Agent::run`].
///
/// Serialized via a private adjacent-tagged wire format
/// (`{"kind": "...", "data": ...}`) so a GUI/IPC consumer can render a
/// discriminated union without a hand-written adapter. The
/// [`AgentError::Responder`] variant carries a
/// `Box<dyn std::error::Error + Send + Sync>` (`HostError`) which is not
/// itself `Serialize`/`Deserialize`; we serialize it as its `Display`
/// string and deserialize it back into a lossless [`StringHostError`]
/// shim. The original error type is therefore not preserved across a
/// round trip — by design, since GUI consumers only need a human-readable
/// message. This is the “stringify” option called out in issue #909.
///
/// `Serialize`/`Deserialize` are implemented by hand (not derived)
/// because serde's adjacent-tagged derive emits a `HostError: Deserialize`
/// bound for the missing-content fallback path, which `Box<dyn Error>`
/// cannot satisfy even when `deserialize_with` is supplied.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The builder was missing a responder.
    #[error("AgentBuilder is missing a responder; call `.responder(...)` before `.build()`")]
    MissingResponder,

    /// The agent loop reached its iteration limit without a final text.
    #[error("agent loop exceeded max_iterations ({0})")]
    MaxIterations(usize),

    /// The LLM asked to call a tool but no [`ToolExecutor`] is wired.
    /// Plug one in via [`AgentBuilder::tool_executor`].
    #[error(
        "tool calls are not supported: no ToolExecutor wired (see AgentBuilder::tool_executor)"
    )]
    ToolsNotSupported,

    /// The loop ended with `LoopOutcome::Failure(reason)`, typically from
    /// an egress hook decision.
    #[error("agent loop failed: {0}")]
    LoopFailure(String),

    /// The loop was halted by an external `LoopSignal::Stop`. The headless
    /// delegate never emits this itself, but it can surface if a custom
    /// delegate is plugged in later.
    #[error("agent loop stopped before producing a final response")]
    Stopped,

    /// A tool approval was requested but the active [`crate::ApprovalPolicy`]
    /// path produced no [`crate::AgentEvent::ApprovalNeeded`] for it.
    ///
    /// This variant only fires when the agent loop surfaces
    /// [`LoopOutcome::NeedApproval`] outside the headless
    /// approval-inbox pipeline; the GUI-friendly path is
    /// [`crate::AgentEvent::ApprovalNeeded`] + [`Agent::respond_to_approval`]
    /// + [`AgentError::ApprovalRejected`].
    ///
    /// Kept for wire backwards compatibility with issue #909 consumers.
    #[deprecated(
        since = "0.0.0-w6",
        note = "GUI hosts should listen for AgentEvent::ApprovalNeeded and reply via Agent::respond_to_approval; only custom tool-dispatch paths that bypass the approval inbox should still surface this variant"
    )]
    #[error("agent loop requested approval, which is not supported in headless mode")]
    ApprovalRequested,

    /// The GUI replied [`crate::ApprovalDecision::Reject`] for a pending
    /// approval prompt (issue #910, GUI blocker B4). The matching
    /// `tool_result` has already been pushed into the conversation as
    /// an error, so callers can inspect history if they want the LLM
    /// view; this surface is the host-facing summary.
    #[error("approval rejected for tool {tool_name}{}", reason.as_deref().map(|r| format!(": {r}")).unwrap_or_default())]
    ApprovalRejected {
        /// Tool whose call the user declined.
        tool_name: String,
        /// Free-form rationale the GUI supplied, if any.
        reason: Option<String>,
    },

    /// The underlying responder (or hook) returned an error.
    #[error("responder error: {0}")]
    Responder(#[source] HostError),
}

/// Private wire format mirror for [`AgentError`]. Drives the manual
/// `Serialize`/`Deserialize` impls so the public type stays untouched.
///
/// Kept private because GUI consumers should match on the JSON `kind`
/// discriminant, not on this Rust type.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
enum AgentErrorWire {
    MissingResponder,
    MaxIterations(usize),
    ToolsNotSupported,
    LoopFailure(String),
    Stopped,
    ApprovalRequested,
    ApprovalRejected {
        tool_name: String,
        reason: Option<String>,
    },
    Responder(String),
}

impl Serialize for AgentError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // The match below names the deprecated `AgentError::ApprovalRequested`
        // variant; allow it locally so the wire mirror keeps full
        // bidirectional coverage without a crate-wide allow.
        #[allow(deprecated)]
        let wire = match self {
            AgentError::MissingResponder => AgentErrorWire::MissingResponder,
            AgentError::MaxIterations(n) => AgentErrorWire::MaxIterations(*n),
            AgentError::ToolsNotSupported => AgentErrorWire::ToolsNotSupported,
            AgentError::LoopFailure(s) => AgentErrorWire::LoopFailure(s.clone()),
            AgentError::Stopped => AgentErrorWire::Stopped,
            AgentError::ApprovalRequested => AgentErrorWire::ApprovalRequested,
            AgentError::ApprovalRejected { tool_name, reason } => {
                AgentErrorWire::ApprovalRejected {
                    tool_name: tool_name.clone(),
                    reason: reason.clone(),
                }
            }
            AgentError::Responder(err) => AgentErrorWire::Responder(err.to_string()),
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AgentErrorWire::deserialize(deserializer)?;
        // Same rationale as the serialize impl above.
        #[allow(deprecated)]
        Ok(match wire {
            AgentErrorWire::MissingResponder => AgentError::MissingResponder,
            AgentErrorWire::MaxIterations(n) => AgentError::MaxIterations(n),
            AgentErrorWire::ToolsNotSupported => AgentError::ToolsNotSupported,
            AgentErrorWire::LoopFailure(s) => AgentError::LoopFailure(s),
            AgentErrorWire::Stopped => AgentError::Stopped,
            AgentErrorWire::ApprovalRequested => AgentError::ApprovalRequested,
            AgentErrorWire::ApprovalRejected { tool_name, reason } => {
                AgentError::ApprovalRejected { tool_name, reason }
            }
            AgentErrorWire::Responder(s) => AgentError::Responder(Box::new(StringHostError(s))),
        })
    }
}

/// Lossless shim that carries a host-error message across a
/// serialize/deserialize round trip. The original concrete error type is
/// erased — see [`AgentError::Responder`] for the rationale.
#[derive(Debug)]
pub struct StringHostError(pub String);

impl std::fmt::Display for StringHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StringHostError {}

/// Static configuration for [`Agent`].
///
/// All fields are optional; defaults are listed below.
#[derive(Default)]
pub struct AgentConfig {
    /// System prompt prepended to every LLM call. When `None`, no system
    /// prompt is injected.
    pub system_prompt: Option<String>,
    /// Per-user model override; passed straight through to the responder
    /// via [`ReasoningContext::model_override`].
    pub model: Option<String>,
    /// Tool definitions advertised to the model. When empty, the agent is
    /// in text-only mode and any tool call from the model triggers
    /// [`AgentError::ToolsNotSupported`] (unless a [`ToolExecutor`] is
    /// nevertheless wired and the model still emits one — same outcome,
    /// the empty advert deters the model up front).
    pub tools: Vec<ToolDefinition>,
    /// Loop tuning. When `None`, [`AgenticLoopConfig::default`] is used
    /// (50 iterations, intent nudges enabled). Not `Clone` upstream, so we
    /// own a fresh value here.
    pub loop_config: Option<AgenticLoopConfig>,
}

/// Headless agent driving [`run_agentic_loop`] with an internal delegate
/// and a configurable [`HookBundle`].
pub struct Agent {
    responder: Arc<dyn AgentResponder>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
    tool_lifecycle_observer: Arc<dyn ToolLifecycleObserver>,
    hooks: HookBundle,
    config: AgentConfig,
    /// Optional cancellation token forwarded to
    /// [`dasclaw_core::agentic_loop::run_agentic_loop`].
    /// When set and tripped, the agentic loop exits with
    /// [`AgentError::Stopped`] on the next signal check. See issue #907.
    cancellation_token: Option<CancellationToken>,
    /// GUI approval policy (issue #910, GUI blocker B4). Defaults to
    /// [`NoApprovalPolicy`] so existing callers keep their zero-event
    /// behaviour bit-for-bit.
    approval_policy: Arc<dyn ApprovalPolicy>,
    /// Pending approval inbox shared with the internal
    /// `SequentialDispatcher` for the lifetime of the agent.
    /// [`Agent::respond_to_approval`] dispatches into this inbox once
    /// the GUI replies.
    approval_inbox: ApprovalInbox,
}

impl Agent {
    /// Start building an agent. See [`AgentBuilder`].
    #[must_use]
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Return a clone of the configured cancellation token, or `None` if
    /// the agent was built without one. Callers (typically a GUI "stop"
    /// button) cancel the returned handle to halt any in-flight
    /// [`Agent::run`] or [`crate::Session::run`] at the next signal
    /// check. See issue #907.
    #[must_use]
    pub fn cancel_handle(&self) -> Option<CancellationToken> {
        self.cancellation_token.clone()
    }

    /// Reply to a pending [`AgentEvent::ApprovalNeeded`] event
    /// (issue #910, GUI blocker B4).
    ///
    /// Looks up the matching request in the agent's approval inbox and
    /// hands the [`ApprovalDecision`] to the headless delegate that is
    /// currently parked on its oneshot receiver. Returns
    /// [`ApprovalDispatchError::Unknown`] when the id is unknown (stale
    /// or duplicate click) and [`ApprovalDispatchError::Closed`] when
    /// the receiver has already been dropped (loop cancellation, agent
    /// shutdown, …).
    pub fn respond_to_approval(
        &self,
        request_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<(), ApprovalDispatchError> {
        self.approval_inbox.dispatch(request_id, decision)
    }

    /// Seed a fresh [`ReasoningContext`] with the agent's static
    /// configuration (system prompt, model override, advertised tools).
    /// Shared by [`Agent::run`] and `dasclaw_session::Session::new` so
    /// the initialisation stays in one place.
    ///
    /// This is part of the support API consumed by the
    /// [`dasclaw_session`](https://docs.rs/dasclaw_session) crate; it is
    /// public so that crate can build a multi-turn facade without
    /// duplicating the seeding logic. End-host code should prefer
    /// [`Agent::run`] or `dasclaw_session::Session`.
    pub fn seed_context(&self, ctx: &mut ReasoningContext) {
        if let Some(ref sp) = self.config.system_prompt {
            ctx.system_prompt = Some(sp.clone());
        }
        if let Some(ref model) = self.config.model {
            ctx.model_override = Some(model.clone());
        }
        if !self.config.tools.is_empty() {
            ctx.available_tools = self.config.tools.clone();
        }
    }

    /// Append `prompt` as a user message to `ctx` and drive the agentic
    /// loop to completion. Called by [`Agent::run`] (after seeding a
    /// fresh context) and by `dasclaw_session::Session::run` (carrying
    /// the session's accumulated context across turns).
    ///
    /// Part of the same support API as [`Agent::seed_context`]; see
    /// that method's docs for the rationale.
    pub async fn run_in_context(
        &self,
        ctx: &mut ReasoningContext,
        prompt: &str,
    ) -> Result<String, AgentError> {
        self.run_in_context_inner(ctx, prompt, None).await
    }

    /// Streaming variant of [`Agent::run_in_context`] (issue #908).
    ///
    /// Events are forwarded to `event_tx` in the order they happen:
    /// [`AgentEvent::TextChunk`] from the responder, then
    /// [`AgentEvent::FinishReason`] at each iteration boundary, with
    /// [`AgentEvent::ToolCallStart`] / [`AgentEvent::ToolResult`]
    /// interleaved when the model calls tools. On return the agent's
    /// final assistant turn is also recorded in `ctx` so a
    /// [`crate::Session`] picks up the history exactly as it does for
    /// non-streaming runs.
    ///
    /// Part of the same support API as [`Agent::seed_context`].
    pub async fn run_in_context_streaming(
        &self,
        ctx: &mut ReasoningContext,
        prompt: &str,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<String, AgentError> {
        self.run_in_context_inner(ctx, prompt, Some(event_tx)).await
    }

    /// Shared loop driver for both streaming and non-streaming runs.
    /// The optional `event_tx` is forwarded to the
    /// [`AgenticLoop`] which routes each iteration to
    /// [`AgentResponder::respond_streaming`] or [`AgentResponder::respond`]
    /// and gates the `ToolCallStart` / `ToolResult` / `FinishReason`
    /// emits.
    async fn run_in_context_inner(
        &self,
        ctx: &mut ReasoningContext,
        prompt: &str,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<String, AgentError> {
        ctx.messages.push(ChatMessage::user(prompt));

        let loop_config = self
            .config
            .loop_config
            .as_ref()
            .map(clone_loop_config)
            .unwrap_or_default();

        // Build the L2 dispatcher once for the lifetime of this run.
        // When no executor is wired, leave the dispatcher unset so the
        // loop emits the `TOOLS_NOT_SUPPORTED_REASON` sentinel.
        let dispatcher: Option<Arc<dyn ToolDispatcher>> = self.tool_executor.as_ref().map(|exec| {
            Arc::new(SequentialDispatcher::new(
                Arc::clone(exec),
                Arc::clone(&self.hooks.egress),
                Arc::new(PolicyApprover::new(
                    Arc::clone(&self.approval_policy),
                    self.approval_inbox.clone(),
                )),
                self.tool_output_sanitizer.clone(),
                Arc::clone(&self.tool_lifecycle_observer),
                event_tx.clone(),
            )) as Arc<dyn ToolDispatcher>
        });

        let agentic_loop = AgenticLoop::new(
            Arc::clone(&self.responder),
            dispatcher,
            self.cancellation_token.clone(),
            event_tx,
        );

        let outcome = agentic_loop
            .run(ctx, &loop_config, &self.hooks)
            .await
            .map_err(AgentError::Responder)?;

        // History continuity: the agentic loop records the final
        // assistant turn into `ctx` (with token usage attached) before
        // returning the response text, mirroring the push that
        // `SequentialDispatcher` already performs on the tool-call
        // branch. That keeps the next call in a multi-turn
        // `Session::run` aware of what the model just said without
        // requiring the caller to plumb usage out of `LoopOutcome`.
        map_outcome(outcome, loop_config.max_iterations)
    }

    /// Run a single user prompt through the loop and return the final
    /// text response.
    pub async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let mut ctx = ReasoningContext::new();
        self.seed_context(&mut ctx);
        self.run_in_context(&mut ctx, prompt).await
    }

    /// Streaming variant of [`Agent::run`] (issue #908, GUI blocker B2).
    ///
    /// Drives one user prompt through the loop while forwarding
    /// [`AgentEvent`]s on `event_tx`. Returns the final assistant text
    /// (the concatenation of every [`AgentEvent::TextChunk`] from the
    /// last iteration), matching [`Agent::run`]'s contract so callers
    /// can opt into streaming without changing their result handling.
    pub async fn run_streaming(
        &self,
        prompt: &str,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<String, AgentError> {
        let mut ctx = ReasoningContext::new();
        self.seed_context(&mut ctx);
        self.run_in_context_streaming(&mut ctx, prompt, event_tx)
            .await
    }
}

/// `AgenticLoopConfig` doesn't impl `Clone`; this preserves the shape.
fn clone_loop_config(src: &AgenticLoopConfig) -> AgenticLoopConfig {
    AgenticLoopConfig {
        max_iterations: src.max_iterations,
        enable_tool_intent_nudge: src.enable_tool_intent_nudge,
        max_tool_intent_nudges: src.max_tool_intent_nudges,
    }
}

/// Fluent builder for [`Agent`].
#[derive(Default)]
pub struct AgentBuilder {
    responder: Option<Arc<dyn AgentResponder>>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
    tool_lifecycle_observer: Option<Arc<dyn ToolLifecycleObserver>>,
    hooks: Option<HookBundle>,
    config: AgentConfig,
    cancellation_token: Option<CancellationToken>,
    approval_policy: Option<Arc<dyn ApprovalPolicy>>,
}

impl AgentBuilder {
    /// Plug in the LLM responder. **Required.**
    #[must_use]
    pub fn responder(mut self, responder: impl AgentResponder + 'static) -> Self {
        self.responder = Some(Arc::new(responder));
        self
    }

    /// Same as [`Self::responder`] but accepts a pre-built `Arc` so callers
    /// can share a single responder across agents.
    #[must_use]
    pub fn responder_arc(mut self, responder: Arc<dyn AgentResponder>) -> Self {
        self.responder = Some(responder);
        self
    }

    /// Plug in a tool executor. Required only if the model is expected to
    /// emit tool calls; otherwise the agent stays in text-only mode.
    #[must_use]
    pub fn tool_executor(mut self, executor: impl ToolExecutor + 'static) -> Self {
        self.tool_executor = Some(Arc::new(executor));
        self
    }

    /// Same as [`Self::tool_executor`] but accepts a pre-built `Arc`.
    #[must_use]
    pub fn tool_executor_arc(mut self, executor: Arc<dyn ToolExecutor>) -> Self {
        self.tool_executor = Some(executor);
        self
    }

    /// Plug in a tool-output sanitizer. When set, every `ToolResult`'s
    /// `content` is run through [`ToolOutputSanitizer::sanitize`] before
    /// being appended to the conversation, so the **next** LLM call sees
    /// the redacted payload. When unset (default), output is forwarded
    /// verbatim — matching the pre-W6.5b behaviour.
    #[must_use]
    pub fn tool_output_sanitizer(mut self, sanitizer: impl ToolOutputSanitizer + 'static) -> Self {
        self.tool_output_sanitizer = Some(Arc::new(sanitizer));
        self
    }

    /// Same as [`Self::tool_output_sanitizer`] but accepts a pre-built
    /// `Arc` so callers can share a single sanitizer across agents.
    #[must_use]
    pub fn tool_output_sanitizer_arc(mut self, sanitizer: Arc<dyn ToolOutputSanitizer>) -> Self {
        self.tool_output_sanitizer = Some(sanitizer);
        self
    }

    /// Attach a production lifecycle observer around real tool execution.
    #[must_use]
    pub fn tool_lifecycle_observer(
        mut self,
        observer: impl ToolLifecycleObserver + 'static,
    ) -> Self {
        self.tool_lifecycle_observer = Some(Arc::new(observer));
        self
    }

    /// Same as [`Self::tool_lifecycle_observer`] but accepts a pre-built
    /// `Arc` so hosts can share an observer across agents.
    #[must_use]
    pub fn tool_lifecycle_observer_arc(mut self, observer: Arc<dyn ToolLifecycleObserver>) -> Self {
        self.tool_lifecycle_observer = Some(observer);
        self
    }

    /// Advertise tool definitions to the model. Without an accompanying
    /// [`Self::tool_executor`], any tool call still maps to
    /// [`AgentError::ToolsNotSupported`].
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.config.tools = tools;
        self
    }

    /// Override the hook environment. Defaults to [`HookBundle::noop`].
    #[must_use]
    pub fn hooks(mut self, hooks: HookBundle) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the system prompt injected into every LLM call.
    #[must_use]
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = Some(prompt.into());
        self
    }

    /// Set the model override forwarded to the responder.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model = Some(model.into());
        self
    }

    /// Replace the loop tuning. When unset, [`AgenticLoopConfig::default`]
    /// is used.
    #[must_use]
    pub fn loop_config(mut self, cfg: AgenticLoopConfig) -> Self {
        self.config.loop_config = Some(cfg);
        self
    }

    /// Wire a [`CancellationToken`] that the headless delegate consults
    /// on each loop signal check. Cancelling the token from outside
    /// (typically via [`Agent::cancel_handle`]) halts an in-flight
    /// [`Agent::run`] or [`crate::Session::run`] with
    /// [`AgentError::Stopped`] at the next iteration boundary. See issue
    /// #907.
    #[must_use]
    pub fn cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Wire an [`ApprovalPolicy`] (issue #910, GUI blocker B4). When
    /// unset, [`NoApprovalPolicy`] is installed so every tool call is
    /// auto-approved and no [`AgentEvent::ApprovalNeeded`] events are
    /// emitted — the pre-B4 contract.
    #[must_use]
    pub fn approval_policy(mut self, policy: Arc<dyn ApprovalPolicy>) -> Self {
        self.approval_policy = Some(policy);
        self
    }

    /// Finalise the agent.
    pub fn build(self) -> Result<Agent, AgentError> {
        let responder = self.responder.ok_or(AgentError::MissingResponder)?;
        let hooks = self.hooks.unwrap_or_else(HookBundle::noop);
        let approval_policy = self
            .approval_policy
            .unwrap_or_else(|| Arc::new(NoApprovalPolicy));
        Ok(Agent {
            responder,
            tool_executor: self.tool_executor,
            tool_output_sanitizer: self.tool_output_sanitizer,
            tool_lifecycle_observer: self
                .tool_lifecycle_observer
                .unwrap_or_else(|| Arc::new(NoopToolLifecycleObserver)),
            hooks,
            config: self.config,
            cancellation_token: self.cancellation_token,
            approval_policy,
            approval_inbox: ApprovalInbox::new(),
        })
    }
}

fn map_outcome(outcome: LoopOutcome, max_iterations: usize) -> Result<String, AgentError> {
    match outcome {
        LoopOutcome::Response(text) => Ok(text),
        LoopOutcome::MaxIterations => Err(AgentError::MaxIterations(max_iterations)),
        LoopOutcome::Failure(reason) if reason == TOOLS_NOT_SUPPORTED_REASON => {
            Err(AgentError::ToolsNotSupported)
        }
        LoopOutcome::Failure(reason) => {
            if let Some(rest) = reason.strip_prefix(APPROVAL_REJECTED_SENTINEL_PREFIX) {
                match serde_json::from_str::<RejectedPayload>(rest) {
                    Ok(payload) => Err(AgentError::ApprovalRejected {
                        tool_name: payload.tool_name,
                        reason: payload.reason,
                    }),
                    // Malformed sentinel: fall back to LoopFailure with
                    // the raw reason so debugging is not silently lost.
                    Err(_) => Err(AgentError::LoopFailure(reason)),
                }
            } else {
                Err(AgentError::LoopFailure(reason))
            }
        }
        LoopOutcome::Stopped => Err(AgentError::Stopped),
        // Custom tool-dispatch paths that surface NeedApproval bypass the
        // approval-inbox pipeline; keep the deprecated wire variant so
        // they keep compiling. New GUI hosts should rely on
        // AgentEvent::ApprovalNeeded + Agent::respond_to_approval.
        #[allow(deprecated)]
        LoopOutcome::NeedApproval(_) => Err(AgentError::ApprovalRequested),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dasclaw_core::messages::{FinishReason, ToolCall};
    use dasclaw_core::response_types::{
        RespondOutput, RespondResult, ResponseMetadata, TokenUsage,
    };

    /// Mock responder driven by a fixed sequence of pre-built outputs.
    struct ScriptedResponder {
        script: tokio::sync::Mutex<Vec<RespondOutput>>,
    }

    impl ScriptedResponder {
        fn new(script: Vec<RespondOutput>) -> Self {
            Self {
                script: tokio::sync::Mutex::new(script),
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

    fn text_output(s: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::Text(s.to_string()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        }
    }

    fn tool_call_output() -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "anything".to_string(),
                    arguments: serde_json::json!({}),
                    reasoning: None,
                }],
                content: None,
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a1_text_response_returns_final_string() {
        let responder = ScriptedResponder::new(vec![text_output("hello world")]);
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build agent");
        let out = agent.run("hi").await.expect("run");
        assert_eq!(out, "hello world");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_929_text_branch_persists_usage_to_context() {
        // #929 step 2: the final assistant ChatMessage recorded on the
        // text branch of run_agentic_loop must carry the per-turn
        // TokenUsage from the LLM call that produced it, so multi-turn
        // Session::run can persist real cost data.
        let usage = TokenUsage {
            input_tokens: 42,
            output_tokens: 7,
            cache_creation_input_tokens: 3,
            cache_read_input_tokens: 1,
        };
        let mut output = text_output("done");
        output.usage = usage;
        let responder = ScriptedResponder::new(vec![output]);
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build agent");
        let mut ctx = ReasoningContext::new();
        let out = agent
            .run_in_context(&mut ctx, "ping")
            .await
            .expect("run_in_context");
        assert_eq!(out, "done");
        let last = ctx
            .messages
            .last()
            .expect("ctx should contain final assistant message");
        assert_eq!(last.usage, Some(usage));
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a1_builder_requires_responder() {
        // `Agent` does not implement `Debug`, so we cannot use `unwrap_err`.
        match Agent::builder().build() {
            Err(AgentError::MissingResponder) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("build should have failed without a responder"),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a1_tool_calls_yield_dedicated_error() {
        let responder = ScriptedResponder::new(vec![tool_call_output()]);
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build");
        let err = agent.run("call a tool please").await.unwrap_err();
        assert!(matches!(err, AgentError::ToolsNotSupported), "got {err:?}");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a1_responder_error_propagates() {
        let responder = ScriptedResponder::new(vec![]); // first call → "script exhausted"
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build");
        let err = agent.run("ping").await.unwrap_err();
        assert!(matches!(err, AgentError::Responder(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a1_system_prompt_propagates_into_context() {
        // Spy responder asserts that the context carries the system prompt
        // and model override the builder configured.
        struct SpyResponder;
        #[async_trait]
        impl AgentResponder for SpyResponder {
            async fn respond(
                &self,
                ctx: &mut ReasoningContext,
            ) -> Result<RespondOutput, HostError> {
                assert_eq!(ctx.system_prompt.as_deref(), Some("be terse"));
                assert_eq!(ctx.model_override.as_deref(), Some("test-model"));
                Ok(text_output("ok"))
            }
        }
        let agent = Agent::builder()
            .responder(SpyResponder)
            .system_prompt("be terse")
            .model("test-model")
            .build()
            .expect("build");
        let out = agent.run("hi").await.expect("run");
        assert_eq!(out, "ok");
    }

    // -----------------------------------------------------------------
    // A2 tests — tool executor wiring
    // -----------------------------------------------------------------

    use dasclaw_core::messages::{ToolDefinition, ToolResult};

    /// Executor that returns a fixed `ToolResult` and counts invocations.
    struct ScriptedExecutor {
        result_content: String,
        calls: tokio::sync::Mutex<Vec<ToolCall>>,
    }

    impl ScriptedExecutor {
        fn new(result_content: impl Into<String>) -> Self {
            Self {
                result_content: result_content.into(),
                calls: tokio::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl ToolExecutor for ScriptedExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
            self.calls.lock().await.push(call.clone());
            Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: self.result_content.clone(),
                is_error: false,
            })
        }
    }

    fn tool_call_output_named(name: &str, id: &str) -> RespondOutput {
        RespondOutput {
            result: RespondResult::ToolCalls {
                tool_calls: vec![ToolCall {
                    id: id.into(),
                    name: name.into(),
                    arguments: serde_json::json!({"x": 1}),
                    reasoning: None,
                }],
                content: Some("let me call it".into()),
            },
            usage: TokenUsage::default(),
            finish_reason: FinishReason::ToolUse,
            metadata: ResponseMetadata::default(),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_tools_advertised_in_context() {
        struct ToolSpy;
        #[async_trait]
        impl AgentResponder for ToolSpy {
            async fn respond(
                &self,
                ctx: &mut ReasoningContext,
            ) -> Result<RespondOutput, HostError> {
                assert_eq!(ctx.available_tools.len(), 1);
                assert_eq!(ctx.available_tools[0].name, "echo");
                Ok(text_output("done"))
            }
        }
        let agent = Agent::builder()
            .responder(ToolSpy)
            .tools(vec![ToolDefinition {
                name: "echo".into(),
                description: "echo back".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .build()
            .expect("build");
        let out = agent.run("hi").await.expect("run");
        assert_eq!(out, "done");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_executor_runs_and_loop_continues() {
        // First LLM turn → tool call; second turn → text response.
        let responder = ScriptedResponder::new(vec![
            tool_call_output_named("echo", "call_1"),
            text_output("all done"),
        ]);
        let executor = ScriptedExecutor::new("echo result");
        let agent = Agent::builder()
            .responder(responder)
            .tool_executor(executor)
            .build()
            .expect("build");
        let out = agent.run("please call echo").await.expect("run");
        assert_eq!(out, "all done");
    }

    struct RecordingLifecycleObserver {
        calls: Arc<tokio::sync::Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl ToolLifecycleObserver for RecordingLifecycleObserver {
        async fn observe(&self, event: ToolLifecycleEvent) -> Result<(), HostError> {
            assert_eq!(event.tool_call_id, "call_1");
            assert_eq!(event.tool_name, "echo");
            let label = match event.stage {
                ToolLifecycleStage::PreToolUse => "pre",
                ToolLifecycleStage::PostToolUse => "post",
            };
            self.calls.lock().await.push(label);
            Ok(())
        }
    }

    struct RecordingOrderExecutor {
        calls: Arc<tokio::sync::Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl ToolExecutor for RecordingOrderExecutor {
        async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
            self.calls.lock().await.push("exec");
            Ok(ToolResult {
                tool_call_id: call.id.clone(),
                name: call.name.clone(),
                content: "echo result".to_string(),
                is_error: false,
            })
        }
    }

    #[tokio::test]
    async fn req_1057_tool_lifecycle_wraps_real_executor_order() {
        // Regression for issue #1057: this must exercise the production
        // Agent -> SequentialDispatcher -> ToolExecutor path. The test
        // records lifecycle callbacks and the actual executor invocation
        // into one log, proving the real order is:
        //   PreToolUse -> execute -> PostToolUse
        let calls = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let responder = ScriptedResponder::new(vec![
            tool_call_output_named("echo", "call_1"),
            text_output("all done"),
        ]);
        let observer = Arc::new(RecordingLifecycleObserver {
            calls: Arc::clone(&calls),
        });
        let executor = RecordingOrderExecutor {
            calls: Arc::clone(&calls),
        };

        let agent = Agent::builder()
            .responder(responder)
            .tool_executor(executor)
            .tool_lifecycle_observer_arc(observer)
            .tools(vec![ToolDefinition {
                name: "echo".into(),
                description: "echo".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .build()
            .expect("build");

        let out = agent.run("please call echo").await.expect("run");
        assert_eq!(out, "all done");

        let actual = calls.lock().await.clone();
        assert_eq!(actual, vec!["pre", "exec", "post"]);
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_executor_error_propagates_as_responder_error() {
        struct ExplodingExecutor;
        #[async_trait]
        impl ToolExecutor for ExplodingExecutor {
            async fn execute(&self, _call: &ToolCall) -> Result<ToolResult, HostError> {
                Err("tool blew up".into())
            }
        }
        let responder = ScriptedResponder::new(vec![tool_call_output_named("boom", "call_x")]);
        let agent = Agent::builder()
            .responder(responder)
            .tool_executor(ExplodingExecutor)
            .build()
            .expect("build");
        let err = agent.run("hi").await.unwrap_err();
        assert!(matches!(err, AgentError::Responder(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_missing_executor_still_yields_tools_not_supported() {
        // Regression: A2 keeps the A1 contract — no executor, model emits
        // a tool call, agent maps to `ToolsNotSupported`.
        let responder = ScriptedResponder::new(vec![tool_call_output_named("x", "call_x")]);
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build");
        let err = agent.run("hi").await.unwrap_err();
        assert!(matches!(err, AgentError::ToolsNotSupported), "got {err:?}");
    }

    // -----------------------------------------------------------------
    // Issue #908 — Agent::run_streaming (GUI blocker B2)
    // -----------------------------------------------------------------

    /// Responder that emits a fixed list of text chunks via
    /// `respond_streaming` and returns the concatenation as the final
    /// `RespondOutput`. Mirrors how `LlmProviderResponder` will forward
    /// real provider chunks at the seam.
    struct ChunkedResponder {
        chunks: Vec<String>,
    }

    #[async_trait]
    impl AgentResponder for ChunkedResponder {
        async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
            Ok(text_output(&self.chunks.concat()))
        }

        async fn respond_streaming(
            &self,
            _ctx: &mut ReasoningContext,
            event_tx: mpsc::Sender<AgentEvent>,
        ) -> Result<RespondOutput, HostError> {
            for chunk in &self.chunks {
                event_tx
                    .send(AgentEvent::TextChunk(chunk.clone()))
                    .await
                    .expect("event_rx alive");
            }
            Ok(text_output(&self.chunks.concat()))
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_b2_run_streaming_orders_chunks_and_finish() {
        let responder = ChunkedResponder {
            chunks: ["Hel", "lo, ", "wor", "ld", "!"]
                .into_iter()
                .map(String::from)
                .collect(),
        };
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build");

        let (tx, mut rx) = mpsc::channel::<AgentEvent>(16);
        let final_text = agent.run_streaming("hi", tx).await.expect("run");

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }

        assert_eq!(events.len(), 6, "5 TextChunk + 1 FinishReason: {events:?}");
        let chunks: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::TextChunk(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(chunks, vec!["Hel", "lo, ", "wor", "ld", "!"]);
        assert!(
            matches!(
                events.last(),
                Some(AgentEvent::FinishReason(FinishReason::Stop))
            ),
            "last must be FinishReason::Stop: {events:?}"
        );
        assert_eq!(final_text, chunks.join(""));
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_b2_run_streaming_default_impl_falls_back_to_respond() {
        // A responder that doesn't override `respond_streaming` must
        // still surface its final text as a single `TextChunk`
        // courtesy of the default trait impl. This is what guarantees
        // pre-#908 adapters keep working untouched.
        let responder = ScriptedResponder::new(vec![text_output("one shot")]);
        let agent = Agent::builder()
            .responder(responder)
            .build()
            .expect("build");

        let (tx, mut rx) = mpsc::channel::<AgentEvent>(4);
        let final_text = agent.run_streaming("hi", tx).await.expect("run");

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }
        assert_eq!(
            events,
            vec![
                AgentEvent::TextChunk("one shot".to_string()),
                AgentEvent::FinishReason(FinishReason::Stop),
            ]
        );
        assert_eq!(final_text, "one shot");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_b2_run_streaming_emits_tool_events_in_order() {
        // First iteration → tool call; second iteration → final text.
        // Expected event order:
        //   FinishReason::ToolUse        (iter 1 LLM)
        //   ToolCallStart { name=echo }  (loop wires tool)
        //   ToolResult   { name=echo }   (executor finished)
        //   TextChunk("all done")        (iter 2 LLM streaming chunk)
        //   FinishReason::Stop           (iter 2 LLM)
        let responder = ScriptedResponder::new(vec![
            tool_call_output_named("echo", "call_1"),
            text_output("all done"),
        ]);
        let executor = ScriptedExecutor::new("echo result");
        let agent = Agent::builder()
            .responder(responder)
            .tool_executor(executor)
            .tools(vec![ToolDefinition {
                name: "echo".into(),
                description: "echo".into(),
                parameters: serde_json::json!({"type": "object"}),
            }])
            .build()
            .expect("build");

        let (tx, mut rx) = mpsc::channel::<AgentEvent>(16);
        let final_text = agent.run_streaming("call echo", tx).await.expect("run");

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            events.push(ev);
        }

        assert!(
            matches!(
                events.first(),
                Some(AgentEvent::FinishReason(FinishReason::ToolUse))
            ),
            "first event = ToolUse FinishReason: {events:?}"
        );
        let tool_start_idx = events
            .iter()
            .position(|e| matches!(e, AgentEvent::ToolCallStart { name, .. } if name == "echo"))
            .expect("ToolCallStart missing");
        let tool_result_idx = events
            .iter()
            .position(|e| matches!(e, AgentEvent::ToolResult { name, content, is_error: false } if name == "echo" && content == "echo result"))
            .expect("ToolResult missing");
        assert!(
            tool_start_idx < tool_result_idx,
            "ToolCallStart must precede ToolResult: {events:?}"
        );
        assert!(
            matches!(
                events.last(),
                Some(AgentEvent::FinishReason(FinishReason::Stop))
            ),
            "last event = Stop FinishReason: {events:?}"
        );
        assert_eq!(final_text, "all done");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_b2_run_streaming_serializes_to_adjacent_tagged_json() {
        // Wire format guard: AgentEvent uses adjacent-tagged JSON so a
        // TypeScript discriminated union renders directly. Pin the
        // four variants so we notice if anything reshapes the wire.
        let chunks = vec![
            AgentEvent::TextChunk("hi".into()),
            AgentEvent::ToolCallStart {
                name: "echo".into(),
                arguments: serde_json::json!({"x": 1}),
            },
            AgentEvent::ToolResult {
                name: "echo".into(),
                content: "ok".into(),
                is_error: false,
            },
            AgentEvent::FinishReason(FinishReason::Stop),
        ];
        let json: Vec<String> = chunks
            .iter()
            .map(|c| serde_json::to_string(c).expect("serialize"))
            .collect();
        assert_eq!(json[0], r#"{"kind":"text_chunk","data":"hi"}"#);
        assert!(
            json[1].starts_with(r#"{"kind":"tool_call_start","data":{"name":"echo""#),
            "got {}",
            json[1]
        );
        assert!(
            json[2].contains(r#""kind":"tool_result""#) && json[2].contains(r#""is_error":false"#),
            "got {}",
            json[2]
        );
        assert_eq!(json[3], r#"{"kind":"finish_reason","data":"stop"}"#);

        // Round-trip every variant.
        for original in chunks {
            let s = serde_json::to_string(&original).expect("ser");
            let back: AgentEvent = serde_json::from_str(&s).expect("de");
            assert_eq!(back, original);
        }
    }
}
