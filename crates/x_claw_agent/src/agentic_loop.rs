//! Unified agentic loop engine.
//!
//! Ported from ironclaw `agent/agentic_loop.rs` as part of Phase 3 Step D-4.
//!
//! The engine runs the core LLM call → tool execution → result processing →
//! context update → repeat cycle. Three consumers (chat dispatcher, job
//! worker, container runtime) customize behavior via the [`LoopDelegate`]
//! trait.
//!
//! ## Route B (scope-narrowing vs. ironclaw's pre-port version)
//!
//! The upstream engine took a `&Reasoning` and passed it through every
//! `call_llm` invocation. That made the engine aware of ironclaw's concrete
//! LLM engine and forced the `x_claw_agent` port to invent a trait facade.
//!
//! Route B removes the `reasoning` parameter entirely: each delegate owns
//! whatever LLM engine it needs internally. The engine only sees
//! `RespondOutput` values — it doesn't know an "LLM" exists.

use async_trait::async_trait;

use crate::hooks::{HookBundle, SafetyDecision};
use crate::intent::{TOOL_INTENT_NUDGE, TRUNCATED_TOOL_CALL_NOTICE, llm_signals_tool_intent};
use crate::messages::{ChatMessage, FinishReason, Role, ToolCall};
use crate::reasoning_ctx::ReasoningContext;
use crate::response_types::{RespondOutput, RespondResult, ResponseMetadata};
use crate::session::PendingApproval;
use crate::traits::HostError;

/// Signal from the delegate indicating how the loop should proceed.
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
#[derive(Debug)]
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

/// Strategy trait — each consumer implements this to customize I/O and
/// lifecycle.
///
/// The shared loop calls these methods at well-defined points. Consumers
/// implement only the behavior that differs between chat, job, and
/// container contexts. The loop itself handles the common logic: tool
/// intent nudge, iteration counting, truncation handling, and the
/// respond → execute → process cycle.
///
/// # `Send + Sync` requirement
///
/// This trait requires `Send + Sync` because the loop accepts
/// `&dyn LoopDelegate`. Delegates using borrowed references (e.g.
/// `ChatDelegate<'a>`) must ensure all borrowed fields are `Send + Sync`.
/// This is a load-bearing constraint: if a delegate needs to be spawned
/// into a detached task, it must use `Arc`-based ownership instead of
/// borrows.
#[async_trait]
pub trait LoopDelegate: Send + Sync {
    /// Called at the start of each iteration. Check for external signals
    /// (cancellation, user messages, stop requests).
    async fn check_signals(&self) -> LoopSignal;

    /// Called before the LLM call. Allows the delegate to refresh tool
    /// definitions, enforce cost guards, or inject messages.
    /// Return `Some(outcome)` to break the loop early.
    async fn before_llm_call(
        &self,
        reason_ctx: &mut ReasoningContext,
        iteration: usize,
    ) -> Option<LoopOutcome>;

    /// Call the LLM and return the result. Delegates own the LLM engine
    /// themselves — the loop doesn't care what produces `RespondOutput`.
    async fn call_llm(
        &self,
        reason_ctx: &mut ReasoningContext,
        iteration: usize,
    ) -> Result<RespondOutput, HostError>;

    /// Handle a text-only response from the LLM.
    /// Return `TextAction::Return` to exit the loop, `TextAction::Continue`
    /// to proceed.
    async fn handle_text_response(
        &self,
        text: &str,
        metadata: ResponseMetadata,
        reason_ctx: &mut ReasoningContext,
    ) -> TextAction;

    /// Execute tool calls and add results to context.
    /// Return `Some(outcome)` to break the loop (e.g. approval needed).
    async fn execute_tool_calls(
        &self,
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        reason_ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError>;

    /// Called when the LLM expresses tool intent without actually calling a
    /// tool. Delegates can use this to emit events or log the nudge for
    /// observability.
    async fn on_tool_intent_nudge(&self, _text: &str, _reason_ctx: &mut ReasoningContext) {}

    /// Called after each successful iteration (no error, no early return).
    async fn after_iteration(&self, _iteration: usize) {}
}

/// Run the unified agentic loop.
///
/// This is the single implementation used by all consumers. The `delegate`
/// provides consumer-specific behavior via the [`LoopDelegate`] trait.
///
/// `hooks` is the environment bundle ([`HookBundle`]). The loop itself
/// calls two of the four hooks directly:
///
/// - `safety.before_prompt` on the most recent user message before each LLM
///   call. A `Block` decision ends the loop with `LoopOutcome::Failure`.
/// - `safety.after_completion` on text-only LLM responses before the
///   delegate sees them. Mutation is in place, so redaction is transparent
///   to the delegate.
///
/// The tool-level safety hooks (`before_tool_call`, `after_tool_output`)
/// and `ApprovalGate::request` are the responsibility of the
/// [`LoopDelegate::execute_tool_calls`] implementation. Delegates typically
/// clone the same `Arc<HookBundle>` at construction time.
pub async fn run_agentic_loop(
    delegate: &dyn LoopDelegate,
    reason_ctx: &mut ReasoningContext,
    config: &AgenticLoopConfig,
    hooks: &HookBundle,
) -> Result<LoopOutcome, HostError> {
    let mut consecutive_tool_intent_nudges: u32 = 0;
    // Accumulates across all iterations (not reset by text responses) so
    // non-consecutive truncations still escalate to force_text.
    let mut truncation_count: u32 = 0;

    for iteration in 1..=config.max_iterations {
        // Check for external signals (stop, cancellation, user messages)
        match delegate.check_signals().await {
            LoopSignal::Continue => {}
            LoopSignal::Stop => return Ok(LoopOutcome::Stopped),
            LoopSignal::InjectMessage(msg) => {
                reason_ctx.messages.push(ChatMessage::user(&msg));
            }
        }

        // Pre-LLM call hook (cost guard, tool refresh, iteration limit
        // nudge)
        if let Some(outcome) = delegate.before_llm_call(reason_ctx, iteration).await {
            return Ok(outcome);
        }

        // Safety hook: scan/redact the most recent user message before the
        // LLM sees it. Mutation is in place so redaction is transparent to
        // the LLM call.
        if let Some(idx) = reason_ctx
            .messages
            .iter()
            .rposition(|m| m.role == Role::User)
        {
            let prompt = &mut reason_ctx.messages[idx].content;
            match hooks.safety.before_prompt(prompt).await {
                // Allow / Redact / Passthrough: continue (single-hook
                // context treats Passthrough as Allow per ADR-146 §2.4).
                Ok(SafetyDecision::Allow)
                | Ok(SafetyDecision::Redact)
                | Ok(SafetyDecision::Passthrough) => {}
                Ok(SafetyDecision::Block { reason }) => {
                    tracing::warn!(iteration, %reason, "safety hook blocked prompt");
                    return Ok(LoopOutcome::Failure(format!(
                        "safety hook blocked prompt: {reason}"
                    )));
                }
                // Ask in a headless agent loop has no UI to render. Fail-Safe:
                // treat as Block. Desktop-client wires real Ask UX in
                // slice D (ADR-146 §2.5).
                Ok(SafetyDecision::Ask { reason, .. }) => {
                    tracing::warn!(
                        iteration,
                        %reason,
                        "safety hook returned Ask in headless context; Fail-Safe block (slice D will wire UI)"
                    );
                    return Ok(LoopOutcome::Failure(format!(
                        "safety hook requires user confirmation (no UI available): {reason}"
                    )));
                }
                // Non-exhaustive guard: future variants added to
                // `SafetyDecision` (which is `#[non_exhaustive]`) must be
                // addressed explicitly at this site; failing closed is
                // Fail-Safe. `#[allow(unreachable_patterns)]` is required
                // because within the defining crate the compiler sees the
                // enum as exhaustive — outside callers will need it.
                #[allow(unreachable_patterns)]
                Ok(other) => {
                    tracing::error!(
                        iteration,
                        decision = ?other,
                        "safety hook returned unhandled SafetyDecision variant; Fail-Safe block"
                    );
                    return Ok(LoopOutcome::Failure(
                        "safety hook returned unsupported decision".to_string(),
                    ));
                }
                Err(e) => {
                    return Err(format!("safety hook error in before_prompt: {e}").into());
                }
            }
        }

        // Call LLM
        let mut output = delegate.call_llm(reason_ctx, iteration).await?;

        // Safety hook: mutate text completions before the delegate sees them.
        // Tool-call responses skip this hook; delegates apply the
        // tool-level hooks themselves inside `execute_tool_calls`.
        if let RespondResult::Text(ref mut text) = output.result
            && let Err(e) = hooks.safety.after_completion(text).await
        {
            return Err(format!("safety hook error in after_completion: {e}").into());
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
                    delegate.on_tool_intent_nudge(&text, reason_ctx).await;
                    reason_ctx.messages.push(ChatMessage::assistant(&text));
                    reason_ctx
                        .messages
                        .push(ChatMessage::user(TOOL_INTENT_NUDGE));
                    delegate.after_iteration(iteration).await;
                    continue;
                }

                // Reset nudge counter since we got a non-intent text
                // response.
                if !llm_signals_tool_intent(&text) {
                    consecutive_tool_intent_nudges = 0;
                }

                match delegate
                    .handle_text_response(&text, output.metadata, reason_ctx)
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
                        reason_ctx.messages.push(ChatMessage::assistant(text));
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
                    delegate.after_iteration(iteration).await;
                    continue;
                }

                consecutive_tool_intent_nudges = 0;
                truncation_count = 0;

                if let Some(outcome) = delegate
                    .execute_tool_calls(tool_calls, content, reason_ctx)
                    .await?
                {
                    return Ok(outcome);
                }
            }
        }

        delegate.after_iteration(iteration).await;
    }

    Ok(LoopOutcome::MaxIterations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{Role, ToolCall, ToolDefinition};
    use crate::response_types::{ResponseAnomaly, TokenUsage};
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

    /// Configurable mock delegate for testing `run_agentic_loop`.
    struct MockDelegate {
        signal: Mutex<LoopSignal>,
        llm_responses: Mutex<Vec<RespondOutput>>,
        tool_exec_count: AtomicUsize,
        tool_exec_outcome: Mutex<Option<LoopOutcome>>,
        iterations_seen: Mutex<Vec<usize>>,
        early_exit: Mutex<Option<(usize, LoopOutcome)>>,
        nudge_count: AtomicUsize,
    }

    impl MockDelegate {
        fn new(responses: Vec<RespondOutput>) -> Self {
            Self {
                signal: Mutex::new(LoopSignal::Continue),
                llm_responses: Mutex::new(responses),
                tool_exec_count: AtomicUsize::new(0),
                tool_exec_outcome: Mutex::new(None),
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
    impl LoopDelegate for MockDelegate {
        async fn check_signals(&self) -> LoopSignal {
            let mut sig = self.signal.lock().await;
            std::mem::replace(&mut *sig, LoopSignal::Continue)
        }

        async fn before_llm_call(
            &self,
            _reason_ctx: &mut ReasoningContext,
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

        async fn call_llm(
            &self,
            _reason_ctx: &mut ReasoningContext,
            _iteration: usize,
        ) -> Result<RespondOutput, HostError> {
            let mut responses = self.llm_responses.lock().await;
            if responses.is_empty() {
                panic!("MockDelegate: no more LLM responses queued");
            }
            Ok(responses.remove(0))
        }

        async fn handle_text_response(
            &self,
            text: &str,
            _metadata: ResponseMetadata,
            _reason_ctx: &mut ReasoningContext,
        ) -> TextAction {
            TextAction::Return(LoopOutcome::Response(text.to_string()))
        }

        async fn execute_tool_calls(
            &self,
            _tool_calls: Vec<ToolCall>,
            _content: Option<String>,
            reason_ctx: &mut ReasoningContext,
        ) -> Result<Option<LoopOutcome>, HostError> {
            self.tool_exec_count.fetch_add(1, Ordering::SeqCst);
            reason_ctx
                .messages
                .push(ChatMessage::user("tool result stub"));
            let outcome = self.tool_exec_outcome.lock().await.take();
            Ok(outcome)
        }

        async fn on_tool_intent_nudge(&self, _text: &str, _reason_ctx: &mut ReasoningContext) {
            self.nudge_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn after_iteration(&self, iteration: usize) {
            self.iterations_seen.lock().await.push(iteration);
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_text_response_returns_immediately() {
        let delegate = MockDelegate::new(vec![text_output("Hello, world!")]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        match outcome {
            LoopOutcome::Response(text) => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected LoopOutcome::Response"),
        }
        assert!(delegate.iterations_seen.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_tool_call_then_text_response() {
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({}),
            reasoning: None,
        };
        let delegate = MockDelegate::new(vec![
            tool_calls_output(vec![tool_call]),
            text_output("Done!"),
        ]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        match outcome {
            LoopOutcome::Response(text) => assert_eq!(text, "Done!"),
            _ => panic!("Expected LoopOutcome::Response"),
        }
        assert_eq!(delegate.tool_exec_count.load(Ordering::SeqCst), 1);
        assert_eq!(*delegate.iterations_seen.lock().await, vec![1]);
    }

    #[tokio::test]
    async fn test_stop_signal_exits_immediately() {
        let delegate =
            MockDelegate::new(vec![text_output("unreachable")]).with_signal(LoopSignal::Stop);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        assert!(matches!(outcome, LoopOutcome::Stopped));
        assert!(delegate.iterations_seen.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_inject_message_adds_user_message() {
        let delegate = MockDelegate::new(vec![text_output("Got it")])
            .with_signal(LoopSignal::InjectMessage("injected prompt".to_string()));
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

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
        impl LoopDelegate for FailOnMalformedResponse {
            async fn check_signals(&self) -> LoopSignal {
                LoopSignal::Continue
            }

            async fn before_llm_call(
                &self,
                _: &mut ReasoningContext,
                _: usize,
            ) -> Option<LoopOutcome> {
                None
            }

            async fn call_llm(
                &self,
                _: &mut ReasoningContext,
                _: usize,
            ) -> Result<RespondOutput, HostError> {
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
                _: &mut ReasoningContext,
            ) -> TextAction {
                assert_eq!(metadata.anomaly, Some(ResponseAnomaly::EmptyToolCompletion));
                TextAction::Return(LoopOutcome::Failure(
                    "malformed tool completion".to_string(),
                ))
            }

            async fn execute_tool_calls(
                &self,
                _: Vec<ToolCall>,
                _: Option<String>,
                _: &mut ReasoningContext,
            ) -> Result<Option<LoopOutcome>, HostError> {
                Ok(None)
            }
        }

        let delegate = FailOnMalformedResponse;
        let mut ctx = ReasoningContext::new();
        let outcome = run_agentic_loop(
            &delegate,
            &mut ctx,
            &AgenticLoopConfig::default(),
            &HookBundle::noop(),
        )
        .await
        .unwrap();

        assert!(
            matches!(outcome, LoopOutcome::Failure(ref reason) if reason == "malformed tool completion")
        );
    }

    #[tokio::test]
    async fn test_max_iterations_reached() {
        struct ContinueDelegate;

        #[async_trait]
        impl LoopDelegate for ContinueDelegate {
            async fn check_signals(&self) -> LoopSignal {
                LoopSignal::Continue
            }
            async fn before_llm_call(
                &self,
                _: &mut ReasoningContext,
                _: usize,
            ) -> Option<LoopOutcome> {
                None
            }
            async fn call_llm(
                &self,
                _: &mut ReasoningContext,
                _: usize,
            ) -> Result<RespondOutput, HostError> {
                Ok(text_output("still working"))
            }
            async fn handle_text_response(
                &self,
                _: &str,
                _: ResponseMetadata,
                ctx: &mut ReasoningContext,
            ) -> TextAction {
                ctx.messages.push(ChatMessage::assistant("still working"));
                TextAction::Continue
            }
            async fn execute_tool_calls(
                &self,
                _: Vec<ToolCall>,
                _: Option<String>,
                _: &mut ReasoningContext,
            ) -> Result<Option<LoopOutcome>, HostError> {
                Ok(None)
            }
        }

        let delegate = ContinueDelegate;
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 3,
            ..Default::default()
        };

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

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
        let delegate = MockDelegate::new(vec![
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

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        assert!(matches!(outcome, LoopOutcome::Response(_)));
        assert_eq!(delegate.nudge_count.load(Ordering::SeqCst), 2);
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
        let delegate = MockDelegate::new(vec![text_output("unreachable")])
            .with_early_exit(1, LoopOutcome::Stopped);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig::default();

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        assert!(matches!(outcome, LoopOutcome::Stopped));
        assert!(delegate.iterations_seen.lock().await.is_empty());
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
        let delegate = MockDelegate::new(vec![truncated_output, text_output("Summarized it.")]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        assert_eq!(delegate.tool_exec_count.load(Ordering::SeqCst), 0);
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
        let delegate = MockDelegate::new(vec![
            make_truncated(),
            make_truncated(),
            make_truncated(),
            text_output("Gave up on tool calls."),
        ]);
        let mut ctx = ReasoningContext::new();
        let config = AgenticLoopConfig {
            max_iterations: 5,
            ..Default::default()
        };

        let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &HookBundle::noop())
            .await
            .unwrap();

        assert!(matches!(outcome, LoopOutcome::Response(_)));
        assert_eq!(delegate.tool_exec_count.load(Ordering::SeqCst), 0);
        assert!(
            ctx.force_text,
            "Should escalate to force_text after repeated truncations"
        );
    }

    // -----------------------------------------------------------------
    // Safety Hook contract tests (Phase 3 Step D-4)
    // -----------------------------------------------------------------

    use crate::hooks::{SafetyDecision, SafetyError, SafetyHook};
    use std::sync::Arc;

    /// Hook that blocks every prompt with a fixed reason.
    struct BlockAllPrompts;

    #[async_trait]
    impl SafetyHook for BlockAllPrompts {
        async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            Ok(SafetyDecision::Block {
                reason: "policy violation".to_string(),
            })
        }
        async fn after_completion(&self, _c: &mut String) -> Result<(), SafetyError> {
            Ok(())
        }
        async fn before_tool_call(
            &self,
            _t: &str,
            _a: &mut serde_json::Value,
        ) -> Result<SafetyDecision, SafetyError> {
            Ok(SafetyDecision::Allow)
        }
        async fn after_tool_output(&self, _t: &str, _o: &mut String) -> Result<(), SafetyError> {
            Ok(())
        }
    }

    /// Hook that redacts secrets in prompts and tags completions.
    struct RedactingHook;

    #[async_trait]
    impl SafetyHook for RedactingHook {
        async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            if prompt.contains("sk-secret") {
                *prompt = prompt.replace("sk-secret", "[REDACTED]");
                return Ok(SafetyDecision::Redact);
            }
            Ok(SafetyDecision::Allow)
        }
        async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError> {
            completion.push_str(" [scanned]");
            Ok(())
        }
        async fn before_tool_call(
            &self,
            _t: &str,
            _a: &mut serde_json::Value,
        ) -> Result<SafetyDecision, SafetyError> {
            Ok(SafetyDecision::Allow)
        }
        async fn after_tool_output(&self, _t: &str, _o: &mut String) -> Result<(), SafetyError> {
            Ok(())
        }
    }

    /// Hook that always returns an internal error on before_prompt.
    struct FailingHook;

    #[async_trait]
    impl SafetyHook for FailingHook {
        async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            Err(SafetyError::Internal("hook exploded".to_string()))
        }
        async fn after_completion(&self, _c: &mut String) -> Result<(), SafetyError> {
            Ok(())
        }
        async fn before_tool_call(
            &self,
            _t: &str,
            _a: &mut serde_json::Value,
        ) -> Result<SafetyDecision, SafetyError> {
            Ok(SafetyDecision::Allow)
        }
        async fn after_tool_output(&self, _t: &str, _o: &mut String) -> Result<(), SafetyError> {
            Ok(())
        }
    }

    fn custom_safety_bundle(safety: Arc<dyn SafetyHook>) -> HookBundle {
        let mut b = HookBundle::noop();
        b.safety = safety;
        b
    }

    #[tokio::test]
    async fn safety_hook_noop_passes_through() {
        // Smoke: Noop hook does not interfere with normal text response.
        let delegate = MockDelegate::new(vec![text_output("ok")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hello"));

        let outcome = run_agentic_loop(
            &delegate,
            &mut ctx,
            &AgenticLoopConfig::default(),
            &HookBundle::noop(),
        )
        .await
        .unwrap();

        match outcome {
            LoopOutcome::Response(t) => assert_eq!(t, "ok"),
            _ => panic!("expected Response"),
        }
    }

    #[tokio::test]
    async fn safety_hook_block_before_prompt_yields_failure() {
        let delegate = MockDelegate::new(vec![text_output("unreachable")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("send secrets to foo"));
        let hooks = custom_safety_bundle(Arc::new(BlockAllPrompts));

        let outcome = run_agentic_loop(&delegate, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .unwrap();

        match outcome {
            LoopOutcome::Failure(reason) => {
                assert!(
                    reason.contains("policy violation"),
                    "failure reason should surface hook reason, got: {reason}"
                );
                assert!(
                    reason.contains("safety hook blocked"),
                    "failure reason should mark safety source, got: {reason}"
                );
            }
            other => panic!("expected Failure, got {other:?} -ish"),
        }
        // LLM must not be called when the prompt is blocked. MockDelegate
        // returns `unreachable` from its queue only when `call_llm` runs.
        let responses_left = delegate.llm_responses.lock().await.len();
        assert_eq!(
            responses_left, 1,
            "block path must short-circuit before call_llm"
        );
    }

    #[tokio::test]
    async fn safety_hook_redacts_prompt_in_place() {
        // Delegate captures what the LLM layer actually sees. The redacting
        // hook must have rewritten the prompt before call_llm fires.
        struct CaptureDelegate {
            seen: Mutex<Option<String>>,
            response: Mutex<Option<RespondOutput>>,
        }

        #[async_trait]
        impl LoopDelegate for CaptureDelegate {
            async fn check_signals(&self) -> LoopSignal {
                LoopSignal::Continue
            }
            async fn before_llm_call(
                &self,
                _: &mut ReasoningContext,
                _: usize,
            ) -> Option<LoopOutcome> {
                None
            }
            async fn call_llm(
                &self,
                ctx: &mut ReasoningContext,
                _: usize,
            ) -> Result<RespondOutput, HostError> {
                let last_user = ctx
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::User)
                    .map(|m| m.content.clone());
                *self.seen.lock().await = last_user;
                Ok(self.response.lock().await.take().expect("one response"))
            }
            async fn handle_text_response(
                &self,
                text: &str,
                _: ResponseMetadata,
                _: &mut ReasoningContext,
            ) -> TextAction {
                TextAction::Return(LoopOutcome::Response(text.to_string()))
            }
            async fn execute_tool_calls(
                &self,
                _: Vec<ToolCall>,
                _: Option<String>,
                _: &mut ReasoningContext,
            ) -> Result<Option<LoopOutcome>, HostError> {
                Ok(None)
            }
        }

        let delegate = CaptureDelegate {
            seen: Mutex::new(None),
            response: Mutex::new(Some(text_output("raw completion"))),
        };
        let mut ctx = ReasoningContext::new();
        ctx.messages
            .push(ChatMessage::user("please use token sk-secret now"));
        let hooks = custom_safety_bundle(Arc::new(RedactingHook));

        let outcome = run_agentic_loop(&delegate, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .unwrap();

        match outcome {
            LoopOutcome::Response(t) => {
                assert_eq!(
                    t, "raw completion [scanned]",
                    "after_completion must mutate the text before the delegate returns"
                );
            }
            _ => panic!("expected Response"),
        }

        let seen = delegate.seen.lock().await.clone().unwrap();
        assert!(
            !seen.contains("sk-secret"),
            "before_prompt must redact in place; LLM saw: {seen}"
        );
        assert!(
            seen.contains("[REDACTED]"),
            "redaction token must be present; LLM saw: {seen}"
        );

        // Redacted content must persist in ctx.messages so the next
        // iteration also sees the clean text.
        let stored = ctx
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .unwrap()
            .content
            .clone();
        assert!(
            !stored.contains("sk-secret"),
            "ctx must retain redacted form, got: {stored}"
        );
    }

    #[tokio::test]
    async fn safety_hook_error_propagates_as_host_error() {
        let delegate = MockDelegate::new(vec![text_output("unreachable")]);
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("anything"));
        let hooks = custom_safety_bundle(Arc::new(FailingHook));

        let err = run_agentic_loop(&delegate, &mut ctx, &AgenticLoopConfig::default(), &hooks)
            .await
            .expect_err("FailingHook must bubble up as HostError");

        let msg = err.to_string();
        assert!(
            msg.contains("before_prompt") && msg.contains("hook exploded"),
            "error must identify hook phase and inner cause, got: {msg}"
        );
    }
}
