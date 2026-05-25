//! ADR-153 §1.1 case **e10 (A4)** — Egress gate Block surfaces as
//! [`AgentError::LoopFailure`] and the LLM is never re-invoked.
//!
//! Coverage:
//! - L6 (Egress) **fail-closed seam** at the `LlmRequest` insertion point.
//! - W6.1 wiring is exercised end-to-end via [`run_with_tools_and_hooks`].
//!
//! This test does not depend on `dasclaw_safety` at all — it injects a
//! degenerate [`BlockingEgressGate`] whose only purpose is to prove the
//! seam returns the Block decision to the caller in the documented
//! [`CliError::Agent(AgentError::LoopFailure(_))`] shape.

use std::sync::Arc;

use dasclaw_cli::{CliError, run_with_tools_and_hooks};
use dasclaw_core::hooks::{
    AutoApproveGate, HookBundle, InMemorySecrets, NoopSandboxExecutor,
};
use dasclaw_core::messages::ToolDefinition;
use dasclaw_core::traits::HostError;
use dasclaw_core::{ToolCall, ToolResult};
use dasclaw_runtime::{AgentError, ToolExecutor};

#[path = "fixtures/safety_fixtures.rs"]
mod safety_fixtures;

use safety_fixtures::{BlockingEgressGate, ScriptedResponder};

struct UnreachableToolExecutor;

#[async_trait::async_trait]
impl ToolExecutor for UnreachableToolExecutor {
    async fn execute(&self, _call: &ToolCall) -> Result<ToolResult, HostError> {
        Err("tool executor must not be reached when egress blocks first".into())
    }
}

#[tokio::test]
async fn req_dasclaw_cli_safety_e10_egress_block_surfaces_as_loop_failure() {
    let responder = ScriptedResponder::with_text("unreached");
    let hooks = HookBundle {
        egress: Arc::new(BlockingEgressGate::new("policy: deny by default")),
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    };

    let result = run_with_tools_and_hooks(
        responder,
        UnreachableToolExecutor,
        Vec::<ToolDefinition>::new(),
        hooks,
        "system",
        "any prompt — gate will short-circuit",
    )
    .await;

    let err = result.expect_err("Block decision must surface as Err");
    match err {
        CliError::Agent(AgentError::LoopFailure(reason)) => {
            assert!(
                reason.contains("policy: deny by default"),
                "LoopFailure reason should propagate the gate's reason, got: {reason}"
            );
        }
        other => panic!("expected CliError::Agent(LoopFailure), got: {other:?}"),
    }
}
