//! End-to-end contract test: `IronclawEgressGate` inside the agentic
//! loop (ADR-148).
//!
//! The unit tests in `src/egress_gate.rs` prove the adapter's `check`
//! method behaves correctly per `EgressKind`. These integration tests
//! prove the full chain: `HookBundle { egress: IronclawEgressGate, .. }`
//! + `AgenticLoop` actually fires the gate at the documented insertion
//!   points (Layer B, between responder output and downstream actions).
//!
//! Why this matters: per `DLP_TESTING_LESSONS_LEARNED.md`, earlier DLP
//! regressions were caused by tests that only covered the happy path or
//! only the adapter — the integration boundary silently shipped raw
//! secrets. A contract test at this layer is the only place that catches
//! "the loop was refactored and the gate stopped firing".
//!
//! W10.1 (ADR-160): the driver is `dasclaw_runtime::AgenticLoop` wired
//! with `StubResponder` (impl `AgentResponder`); no `ToolDispatcher`
//! is needed because these scenarios never emit tool calls.
//!
//! Gated on `egress-gate` feature so this test only builds when the
//! adapter itself is compiled in.

#![cfg(feature = "egress-gate")]

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::agentic_loop::AgenticLoopConfig;
use dasclaw_core::messages::FinishReason;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::{ChatMessage, HookBundle, HostError};
use dasclaw_runtime::{AgentResponder, AgenticLoop, LoopOutcome};
use dasclaw_safety::egress_gate::IronclawEgressGate;
use dasclaw_safety::{SafetyConfig, SafetyLayer};
use tokio::sync::Mutex;

/// Minimal [`AgentResponder`] that returns pre-canned LLM responses.
struct StubResponder {
    queued: Mutex<Vec<RespondOutput>>,
}

impl StubResponder {
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

    async fn remaining(&self) -> usize {
        self.queued.lock().await.len()
    }
}

#[async_trait]
impl AgentResponder for StubResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let mut q = self.queued.lock().await;
        let next = q
            .first()
            .cloned()
            .ok_or_else(|| -> HostError { "StubResponder exhausted".into() })?;
        q.remove(0);
        Ok(next)
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
    b.egress = Arc::new(IronclawEgressGate::new(layer));
    b
}

async fn run_with(
    responder: Arc<StubResponder>,
    ctx: &mut ReasoningContext,
    bundle: &HookBundle,
) -> Result<LoopOutcome, HostError> {
    let config = AgenticLoopConfig::default();
    AgenticLoop::new(responder, None, None, None)
        .run(ctx, &config, bundle)
        .await
}

/// LLM returns a completion containing a fake OpenAI key. The outcome
/// reaching the caller MUST NOT contain the raw secret — the egress
/// gate fires inside the loop and scrubs (or blocks) before the
/// completion is surfaced.
#[tokio::test]
async fn run_agentic_loop_with_egress_gate_scrubs_secret_from_completion() {
    let raw_secret = format!("sk-{}", "Z".repeat(48));
    let completion = format!("here is the key: {raw_secret}");

    let responder = Arc::new(StubResponder::with_text(completion.clone()));
    let bundle = bundle_with(safety_layer());
    let mut ctx = ReasoningContext::new();
    ctx.messages.push(ChatMessage::user("give me the key"));

    let outcome = run_with(responder, &mut ctx, &bundle)
        .await
        .expect("loop should not bubble HostError");

    match outcome {
        LoopOutcome::Response(text) => {
            assert!(
                !text.contains(&raw_secret),
                "raw secret leaked through the egress chain: {text}"
            );
        }
        LoopOutcome::Failure(reason) => {
            assert!(
                !reason.contains(&raw_secret),
                "failure reason must not echo the raw secret: {reason}"
            );
        }
        other => panic!("expected Response or Failure, got {other:?}"),
    }
}

/// Last user message carries a secret. The egress gate must fire on
/// `LlmRequest` and short-circuit the loop to `Failure` — the LLM is
/// never called.
#[tokio::test]
async fn run_agentic_loop_with_egress_gate_blocks_leaky_prompt() {
    let raw_secret = format!("sk-{}", "Q".repeat(48));
    let responder = Arc::new(StubResponder::with_text("unreached"));
    let bundle = bundle_with(safety_layer());

    let mut ctx = ReasoningContext::new();
    ctx.messages
        .push(ChatMessage::user(format!("please use {raw_secret}")));

    let outcome = run_with(Arc::clone(&responder), &mut ctx, &bundle)
        .await
        .expect("Block should surface as Failure, not Err");

    match outcome {
        LoopOutcome::Failure(reason) => {
            assert!(
                !reason.contains(&raw_secret),
                "block reason must not echo the raw secret: {reason}"
            );
        }
        other => panic!("expected Failure, got {other:?}"),
    }

    // Confirm the LLM was not dialed: the queued response is still there.
    assert_eq!(responder.remaining().await, 1);
}

/// Clean prompt + clean completion — the gate must be transparent.
/// Guards against a regression where the gate wrongly rewrites benign text.
#[tokio::test]
async fn run_agentic_loop_with_egress_gate_transparent_on_clean_input() {
    let responder = Arc::new(StubResponder::with_text("all good"));
    let bundle = bundle_with(safety_layer());
    let mut ctx = ReasoningContext::new();
    ctx.messages.push(ChatMessage::user("how are things?"));

    let outcome = run_with(responder, &mut ctx, &bundle)
        .await
        .expect("loop should succeed");

    match outcome {
        LoopOutcome::Response(text) => assert_eq!(text, "all good"),
        other => panic!("expected Response, got {other:?}"),
    }
}
