//! ADR-153 §1.1 case **e7 (A1)** — `IronclawEgressGate` blocks an LLM
//! request whose user prompt contains a literal API key, and the raw
//! secret never appears in the surfaced [`CliError`].
//!
//! Coverage:
//! - L6 (Egress) **`LlmRequest` insertion point** with the production
//!   gate implementation (`dasclaw_safety::egress_gate::IronclawEgressGate`).
//! - W6.1 wiring (`run_with_tools_and_hooks`) **plus** real
//!   `SafetyLayer` (`dasclaw_safety`) — proves the seam carries Block
//!   reasons through `AgentError::LoopFailure` without leaking the
//!   payload.
//!
//! Companion to `crates/dasclaw_safety/tests/egress_gate_integration.rs`:
//! that suite proves the gate works against `run_agentic_loop` directly;
//! this one proves the same gate works through the higher-level CLI
//! entry point added in W6.1.

use std::sync::Arc;

use dasclaw_cli::{CliError, run_with_tools_and_hooks};
use dasclaw_core::hooks::{AutoApproveGate, HookBundle, InMemorySecrets, NoopSandboxExecutor};
use dasclaw_core::messages::ToolDefinition;
use dasclaw_core::traits::HostError;
use dasclaw_core::{ToolCall, ToolResult};
use dasclaw_runtime::{AgentError, ToolExecutor};
use dasclaw_safety::egress_gate::IronclawEgressGate;
use dasclaw_safety::{SafetyConfig, SafetyLayer};

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::ScriptedResponder;

struct UnreachableToolExecutor;

#[async_trait::async_trait]
impl ToolExecutor for UnreachableToolExecutor {
    async fn execute(&self, _call: &ToolCall) -> Result<ToolResult, HostError> {
        Err("tool executor must not be reached when egress blocks first".into())
    }
}

fn safety_layer() -> Arc<SafetyLayer> {
    Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 10_000,
        injection_check_enabled: true,
    }))
}

#[tokio::test]
async fn req_dasclaw_cli_safety_e7_llm_request_blocks_user_prompt_with_secret() {
    // Fake OpenAI-style key. The exact pattern matters: the SafetyLayer
    // secret detector keys off `sk-[A-Za-z0-9]{20+}` style tokens.
    let raw_secret = format!("sk-{}", "A".repeat(48));
    let user_prompt = format!("please call the API with key {raw_secret}");

    let hooks = HookBundle {
        egress: Arc::new(IronclawEgressGate::new(safety_layer())),
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    };

    let result = run_with_tools_and_hooks(
        ScriptedResponder::with_text("unreached"),
        UnreachableToolExecutor,
        Vec::<ToolDefinition>::new(),
        hooks,
        "system",
        user_prompt.as_str(),
    )
    .await;

    let err = result.expect_err("secret in user prompt must surface as Err");
    let rendered = err.to_string();
    assert!(
        !rendered.contains(&raw_secret),
        "raw secret leaked through CliError rendering: {rendered}"
    );

    match err {
        CliError::Agent(AgentError::LoopFailure(reason)) => {
            assert!(
                !reason.contains(&raw_secret),
                "LoopFailure reason must not echo the raw secret: {reason}"
            );
        }
        // Redact-then-continue is also a valid fail-safe outcome at this
        // layer (the agent might continue with a sanitized prompt and
        // return a benign reply). We only require: (a) Err or sanitized
        // success, and (b) the raw secret is never echoed back.
        other => panic!(
            "expected CliError::Agent(LoopFailure) when the prompt carries a raw secret, got: {other:?}"
        ),
    }
}
