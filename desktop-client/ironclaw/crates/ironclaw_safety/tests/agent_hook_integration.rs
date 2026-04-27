//! End-to-end contract test: `IronclawSafetyHook` inside `run_agentic_loop`.
//!
//! The unit tests in `src/agent_hook.rs` prove the adapter's four methods
//! behave correctly in isolation. These integration tests prove the full
//! chain: `HookBundle { safety: IronclawSafetyHook, .. }` + `run_agentic_loop`
//! actually fires the hooks at the documented insertion points.
//!
//! Why this matters: per `DLP_TESTING_LESSONS_LEARNED.md`, earlier DLP
//! regressions were caused by tests that only covered the happy path or
//! only the adapter — the integration boundary silently shipped raw
//! secrets. A contract test at this layer is the only place that catches
//! "the loop was refactored and the hook stopped firing".
//!
//! Gated on `agent-hook` feature so this test only builds when the adapter
//! itself is compiled in.

#![cfg(feature = "agent-hook")]

use std::sync::Arc;

use async_trait::async_trait;
use ironclaw_safety::agent_hook::IronclawSafetyHook;
use ironclaw_safety::{SafetyConfig, SafetyLayer};
use tokio::sync::Mutex;
use x_claw_agent::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
use x_claw_agent::messages::FinishReason;
use x_claw_agent::reasoning_ctx::ReasoningContext;
use x_claw_agent::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use x_claw_agent::{ChatMessage, HookBundle, HostError, ToolCall};

/// Minimal `LoopDelegate` that returns pre-canned LLM responses.
///
/// Kept deliberately tiny — just enough to drive `run_agentic_loop` one
/// turn so we can observe what `handle_text_response` receives after the
/// `after_completion` hook runs.
struct StubDelegate {
    queued: Mutex<Vec<RespondOutput>>,
}

impl StubDelegate {
    fn with_text(text: impl Into<String>) -> Self {
        Self {
            queued: Mutex::new(vec![RespondOutput {
                result: RespondResult::Text(text.into()),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                metadata: ResponseMetadata::default(),
            }]),
        }
    }
}

#[async_trait]
impl LoopDelegate for StubDelegate {
    async fn check_signals(&self) -> LoopSignal {
        LoopSignal::Continue
    }

    async fn before_llm_call(
        &self,
        _reason_ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Option<LoopOutcome> {
        None
    }

    async fn call_llm(
        &self,
        _reason_ctx: &mut ReasoningContext,
        _iteration: usize,
    ) -> Result<RespondOutput, HostError> {
        let mut q = self.queued.lock().await;
        assert!(!q.is_empty(), "StubDelegate exhausted");
        Ok(q.remove(0))
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
        _reason_ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        Ok(None)
    }
}

fn safety_layer() -> Arc<SafetyLayer> {
    Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 10_000,
        injection_check_enabled: true,
    }))
}

fn bundle_with(layer: Arc<SafetyLayer>) -> HookBundle {
    let mut b = HookBundle::noop();
    b.safety = Arc::new(IronclawSafetyHook::new(layer));
    b
}

/// LLM returns a completion containing a fake OpenAI key. The outcome
/// reaching the caller MUST NOT contain the raw secret — `after_completion`
/// fires inside the loop and scrubs it before `handle_text_response` sees
/// it.
#[tokio::test]
async fn run_agentic_loop_with_safety_hook_scrubs_secret_from_completion() {
    let raw_secret = format!("sk-{}", "Z".repeat(48));
    let completion = format!("here is the key: {raw_secret}");

    let delegate = StubDelegate::with_text(completion.clone());
    let bundle = bundle_with(safety_layer());
    let mut ctx = ReasoningContext::new();
    ctx.messages.push(ChatMessage::user("give me the key"));
    let config = AgenticLoopConfig::default();

    let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &bundle)
        .await
        .expect("loop should succeed");

    let LoopOutcome::Response(text) = outcome else {
        panic!("expected Response, got {outcome:?}");
    };

    assert!(
        !text.contains(&raw_secret),
        "raw secret leaked through the hook chain: {text}"
    );
}

/// Last user message carries a secret. `before_prompt` must fire and
/// short-circuit the loop to `Failure` — the LLM is never called.
#[tokio::test]
async fn run_agentic_loop_with_safety_hook_blocks_leaky_prompt() {
    let raw_secret = format!("sk-{}", "Q".repeat(48));
    let delegate = StubDelegate::with_text("unreached");
    let bundle = bundle_with(safety_layer());

    let mut ctx = ReasoningContext::new();
    ctx.messages
        .push(ChatMessage::user(format!("please use {raw_secret}")));
    let config = AgenticLoopConfig::default();

    let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &bundle)
        .await
        .expect("Block should surface as Failure, not Err");

    match outcome {
        LoopOutcome::Failure(reason) => {
            // Reason should be a user-facing string without the raw
            // secret in it.
            assert!(
                !reason.contains(&raw_secret),
                "block reason must not echo the raw secret: {reason}"
            );
        }
        other => panic!("expected Failure, got {other:?}"),
    }

    // Confirm the LLM was not dialed: the queued response is still there.
    assert_eq!(delegate.queued.lock().await.len(), 1);
}

/// Clean prompt + clean completion — the hook pipeline must be transparent.
/// Guards against a regression where the hook wrongly rewrites benign text.
#[tokio::test]
async fn run_agentic_loop_with_safety_hook_transparent_on_clean_input() {
    let delegate = StubDelegate::with_text("all good");
    let bundle = bundle_with(safety_layer());
    let mut ctx = ReasoningContext::new();
    ctx.messages.push(ChatMessage::user("how are things?"));
    let config = AgenticLoopConfig::default();

    let outcome = run_agentic_loop(&delegate, &mut ctx, &config, &bundle)
        .await
        .expect("loop should succeed");

    match outcome {
        LoopOutcome::Response(text) => assert_eq!(text, "all good"),
        other => panic!("expected Response, got {other:?}"),
    }
}
