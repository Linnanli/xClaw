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
//!   `AgentError`), wraps [`dasclaw_core::agentic_loop::run_agentic_loop`]
//!   with an internal `HeadlessDelegate` and a configurable `HookBundle`.
//!   The narrow [`AgentResponder`] trait is the single seam where any LLM
//!   adapter plugs in.
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
//! `RespondOutput`, so the seam here mirrors `LoopDelegate::call_llm`'s
//! signature one-to-one and stays single-method to keep the wire-up cost
//! for adapter authors trivial.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
use dasclaw_core::egress_apply::{EgressApply, apply_egress_decision};
use dasclaw_core::hooks::{EgressKind, HookBundle};
use dasclaw_core::messages::{ChatMessage, ToolCall, ToolDefinition, ToolResult};
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

/// Narrow tool-output sanitization seam used by [`Agent`]
/// (ADR-153 §1.1 B6 / e22 wiring).
///
/// Sits between [`ToolExecutor::execute`] and the message-history push,
/// so the redacted content is what the **next** LLM iteration sees in
/// its `tool_result` block. Implementations typically delegate to a
/// safety crate (e.g. `dasclaw_safety::SafetyLayer::sanitize_for_stash`),
/// but the runtime stays safety-stack-agnostic: any `Fn`-like wrapper
/// works as long as it returns the post-redaction string.
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

/// Errors surfaced by [`Agent::run`].
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
        if !self.config.tools.is_empty() {
            ctx.available_tools = self.config.tools.clone();
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
            tool_executor: self.tool_executor.clone(),
            hooks: self.hooks.clone(),
            tool_output_sanitizer: self.tool_output_sanitizer.clone(),
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
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
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

    /// Finalise the agent.
    pub fn build(self) -> Result<Agent, AgentError> {
        let responder = self.responder.ok_or(AgentError::MissingResponder)?;
        let hooks = self.hooks.unwrap_or_else(HookBundle::noop);
        Ok(Agent {
            responder,
            tool_executor: self.tool_executor,
            tool_output_sanitizer: self.tool_output_sanitizer,
            hooks,
            config: self.config,
        })
    }
}

/// Internal `LoopDelegate` that bridges the agentic loop to a single
/// [`AgentResponder`]. The agent has already seeded the reasoning
/// context with the user prompt, system prompt, model override and
/// advertised tools before the loop starts, so the delegate only needs
/// the responder and an optional tool executor.
struct HeadlessDelegate {
    responder: Arc<dyn AgentResponder>,
    tool_executor: Option<Arc<dyn ToolExecutor>>,
    hooks: HookBundle,
    tool_output_sanitizer: Option<Arc<dyn ToolOutputSanitizer>>,
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
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        // No executor → emit the sentinel and let `map_outcome` raise
        // `AgentError::ToolsNotSupported`. This keeps the A1 contract
        // intact when callers use text-only agents.
        let Some(executor) = self.tool_executor.as_ref() else {
            return Ok(Some(LoopOutcome::Failure(
                TOOLS_NOT_SUPPORTED_REASON.to_string(),
            )));
        };

        push_assistant_tool_calls(ctx, content, &tool_calls);
        for call in &tool_calls {
            // ADR-148 Layer B + ADR-153 §1.1 e8/A2:
            // Pre-execute egress gate scans serialised tool arguments.
            // `Block` → executor is never invoked; an is_error
            // tool_result is fed back to the model so it can react.
            // `Redact` → callers that want argument rewriting should
            // implement their own ToolExecutor wrapper; here we honour
            // the sanitized payload only as a tracing hint and proceed.
            let args_payload =
                serde_json::to_string(&call.arguments).unwrap_or_else(|_| String::from("{}"));
            let kind = EgressKind::ToolExecution {
                tool: call.name.clone(),
            };
            let mut args_buf = args_payload;
            let pre_label = format!("ToolExecution:{}", call.name);
            let decision = self.hooks.egress.check(&kind, &args_buf).await;
            if let EgressApply::Halt(reason) =
                apply_egress_decision(decision, &mut args_buf, &pre_label)
            {
                tracing::warn!(tool = %call.name, %reason, "egress gate blocked tool call");
                let error_body = format!("Error: {reason}");
                ctx.messages
                    .push(ChatMessage::tool_result(&call.id, &call.name, &error_body));
                continue;
            }

            let result = executor.execute(call).await?;

            // ADR-148 Layer B + ADR-153 §1.1 e15/A9:
            // Post-execute egress gate sanitises tool output before it
            // becomes part of ReasoningContext. `Redact` swaps content
            // in place; `Block` substitutes a `[redacted: …]` placeholder
            // so the conversation keeps making progress without leaking.
            let mut content_buf = result.content.clone();
            let post_label = format!("ToolOutput:{}", result.name);
            let decision = self
                .hooks
                .egress
                .check(&EgressKind::UserDisplay, &content_buf)
                .await;
            let post_gate_content = match apply_egress_decision(
                decision,
                &mut content_buf,
                &post_label,
            ) {
                EgressApply::Continue => content_buf,
                EgressApply::Halt(reason) => {
                    tracing::warn!(tool = %result.name, %reason, "egress gate blocked tool output");
                    format!("[redacted: {reason}]")
                }
            };

            // ADR-153 §1.1 e22/B6: chain a second sanitizer seam after the
            // egress gate. Callers can inject SafetyLayer::sanitize_for_stash
            // (or any other strategy) without touching the hook stack.
            let pushed_content = match self.tool_output_sanitizer.as_ref() {
                Some(sanitizer) => sanitizer.sanitize(&result.name, &post_gate_content),
                None => post_gate_content,
            };
            ctx.messages.push(ChatMessage::tool_result(
                &result.tool_call_id,
                &result.name,
                &pushed_content,
            ));
        }
        Ok(None)
    }
}

/// Record the assistant's tool-call turn so the next LLM call sees the
/// canonical Anthropic-style assistant + tool_result pairing.
fn push_assistant_tool_calls(
    ctx: &mut ReasoningContext,
    content: Option<String>,
    tool_calls: &[ToolCall],
) {
    ctx.messages.push(ChatMessage::assistant_with_tool_calls(
        content,
        tool_calls.to_vec(),
    ));
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
}
