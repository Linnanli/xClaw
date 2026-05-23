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
//! ## Scope of this sub-step (A1)
//!
//! - Defines the public shape (`Agent`, `AgentBuilder`, `AgentConfig`,
//!   `AgentError`).
//! - Wraps [`dasclaw_core::agentic_loop::run_agentic_loop`] with an
//!   internal `HeadlessDelegate` and `HookBundle::noop()`.
//! - Exposes a narrow [`AgentResponder`] trait — the single seam where a
//!   concrete LLM provider gets plugged in. The runtime calls it once per
//!   loop iteration to obtain a `RespondOutput`.
//! - Tool execution is **not** wired yet: if the responder returns
//!   `ToolCalls`, the loop ends with `AgentError::ToolsNotSupported`. The
//!   tool-binding builder methods (`tools_default`, `tools_with`) land in
//!   sub-step A2 together with the `dasclaw_llm_provider` adapter.
//!
//! ## Why a new `AgentResponder` trait instead of `LlmCompleter`
//!
//! [`dasclaw_core::traits::LlmCompleter`] is a **text-only** facade
//! intended for compaction summaries and similar one-shot helpers — it
//! returns a `String`, not a `RespondOutput`, so it cannot carry tool
//! calls or finish reasons. The agent loop fundamentally needs
//! `RespondOutput`, so the seam here mirrors `LoopDelegate::call_llm`'s
//! signature one-to-one and stays single-method to keep the wire-up cost
//! for adapter authors trivial.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ChatMessage, ToolCall};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, ResponseMetadata};
use dasclaw_core::traits::HostError;

/// Narrow LLM seam used by [`Agent`].
///
/// Adapters wrap a concrete LLM client (`dasclaw_llm_provider`, a mock,
/// a recorder, …) and translate from the provider-native response into a
/// [`RespondOutput`]. The runtime calls `respond` once per iteration of
/// the agentic loop.
#[async_trait]
pub trait AgentResponder: Send + Sync {
    /// Produce the next response for the given context.
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError>;
}

/// Errors surfaced by [`Agent::run`].
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The builder was missing a responder.
    #[error("AgentBuilder is missing a responder; call `.responder(...)` before `.build()`")]
    MissingResponder,

    /// The agent loop reached its iteration limit without a final text.
    #[error("agent loop exceeded max_iterations ({0})")]
    MaxIterations(usize),

    /// The LLM asked to call a tool but no tools are wired (sub-step A1).
    /// Tool support lands in sub-step A2.
    #[error("tool calls are not supported yet (ADR-153 step 2 sub-step A2)")]
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

    /// A tool approval was requested. Approval flow is delegate-specific
    /// and the headless delegate does not implement it.
    #[error("agent loop requested approval, which is not supported in headless mode")]
    ApprovalRequested,

    /// The underlying responder (or hook) returned an error.
    #[error("responder error: {0}")]
    Responder(#[source] HostError),
}

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
    /// Loop tuning. When `None`, [`AgenticLoopConfig::default`] is used
    /// (50 iterations, intent nudges enabled). Not `Clone` upstream, so we
    /// own a fresh value here.
    pub loop_config: Option<AgenticLoopConfig>,
}

/// Headless agent driving [`run_agentic_loop`] with an internal delegate
/// and a configurable [`HookBundle`].
pub struct Agent {
    responder: Arc<dyn AgentResponder>,
    hooks: HookBundle,
    config: AgentConfig,
}

impl Agent {
    /// Start building an agent. See [`AgentBuilder`].
    #[must_use]
    pub fn builder() -> AgentBuilder {
        AgentBuilder::default()
    }

    /// Run a single user prompt through the loop and return the final
    /// text response.
    pub async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let mut ctx = ReasoningContext::new();
        if let Some(ref sp) = self.config.system_prompt {
            ctx.system_prompt = Some(sp.clone());
        }
        if let Some(ref model) = self.config.model {
            ctx.model_override = Some(model.clone());
        }
        ctx.messages.push(ChatMessage::user(prompt));

        let loop_config = self
            .config
            .loop_config
            .as_ref()
            .map(clone_loop_config)
            .unwrap_or_default();

        let delegate = HeadlessDelegate {
            responder: Arc::clone(&self.responder),
        };

        let outcome = run_agentic_loop(&delegate, &mut ctx, &loop_config, &self.hooks)
            .await
            .map_err(AgentError::Responder)?;

        map_outcome(outcome, loop_config.max_iterations)
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
    hooks: Option<HookBundle>,
    config: AgentConfig,
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

    /// Finalise the agent.
    pub fn build(self) -> Result<Agent, AgentError> {
        let responder = self.responder.ok_or(AgentError::MissingResponder)?;
        let hooks = self.hooks.unwrap_or_else(HookBundle::noop);
        Ok(Agent {
            responder,
            hooks,
            config: self.config,
        })
    }
}

/// Internal `LoopDelegate` that bridges the agentic loop to a single
/// [`AgentResponder`]. The agent has already seeded the reasoning
/// context with the user prompt, system prompt and model override before
/// the loop starts, so the delegate keeps no extra state.
struct HeadlessDelegate {
    responder: Arc<dyn AgentResponder>,
}

#[async_trait]
impl LoopDelegate for HeadlessDelegate {
    async fn check_signals(&self) -> LoopSignal {
        LoopSignal::Continue
    }

    async fn before_llm_call(
        &self,
        _ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Option<LoopOutcome> {
        None
    }

    async fn call_llm(
        &self,
        ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Result<RespondOutput, HostError> {
        self.responder.respond(ctx).await
    }

    async fn handle_text_response(
        &self,
        text: &str,
        _metadata: ResponseMetadata,
        _ctx: &mut ReasoningContext,
    ) -> TextAction {
        TextAction::Return(LoopOutcome::Response(text.to_string()))
    }

    async fn execute_tool_calls(
        &self,
        _tool_calls: Vec<ToolCall>,
        _content: Option<String>,
        _ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        // Tool wiring lands in ADR-153 step 2 sub-step A2. Returning
        // Failure here makes the loop end deterministically; `Agent::run`
        // maps it to `AgentError::ToolsNotSupported` via the dedicated
        // failure-reason match below.
        Ok(Some(LoopOutcome::Failure(
            TOOLS_NOT_SUPPORTED_REASON.to_string(),
        )))
    }
}

/// Sentinel string emitted by [`HeadlessDelegate::execute_tool_calls`]
/// and re-mapped to [`AgentError::ToolsNotSupported`] by [`map_outcome`].
const TOOLS_NOT_SUPPORTED_REASON: &str = "headless-agent::tools-not-supported";

fn map_outcome(outcome: LoopOutcome, max_iterations: usize) -> Result<String, AgentError> {
    match outcome {
        LoopOutcome::Response(text) => Ok(text),
        LoopOutcome::MaxIterations => Err(AgentError::MaxIterations(max_iterations)),
        LoopOutcome::Failure(reason) if reason == TOOLS_NOT_SUPPORTED_REASON => {
            Err(AgentError::ToolsNotSupported)
        }
        LoopOutcome::Failure(reason) => Err(AgentError::LoopFailure(reason)),
        LoopOutcome::Stopped => Err(AgentError::Stopped),
        LoopOutcome::NeedApproval(_) => Err(AgentError::ApprovalRequested),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dasclaw_core::messages::{FinishReason, ToolCall};
    use dasclaw_core::response_types::{RespondResult, TokenUsage};

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
}
