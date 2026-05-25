//! W6.1 wiring smoke test for ADR-153-cli-test-matrix §1.3 G1.
//!
//! Proves that:
//!   1. `dasclaw_cli::run_with_tools_and_hooks` compiles and is reachable
//!      from an external crate (the test crate).
//!   2. Passing `HookBundle::noop()` yields the exact same observable
//!      reply as `run_with_tools` does for the same `EchoResponder` —
//!      meaning the new entry is a strict superset, not a behavioural
//!      fork.
//!   3. A custom `EgressGate` implementation can be plugged into the
//!      `HookBundle.egress` slot. Real per-layer assertions (e7–e16)
//!      land in W6.2+ as separate PRs.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_cli::{EchoResponder, run_with_tools, run_with_tools_and_hooks};
use dasclaw_core::hooks::{
    AutoApproveGate, EgressDecision, EgressGate, EgressKind, HookBundle, InMemorySecrets,
    NoopSandboxExecutor,
};
use dasclaw_core::messages::ToolDefinition;
use dasclaw_core::traits::HostError;
use dasclaw_core::{ToolCall, ToolResult};
use dasclaw_runtime::ToolExecutor;

/// Minimal no-op tool executor. Tools are never invoked by
/// [`EchoResponder`] (it returns plain text), so this just needs to
/// satisfy the trait bound on `run_with_tools_and_hooks`.
struct NoopToolExecutor;

#[async_trait]
impl ToolExecutor for NoopToolExecutor {
    async fn execute(&self, _call: &ToolCall) -> Result<ToolResult, HostError> {
        // Unreachable from EchoResponder; if a future change starts
        // calling tools we want a clear test failure rather than silent
        // success.
        Err("NoopToolExecutor invoked unexpectedly".into())
    }
}

/// Allow-all egress gate. Exists only to prove the trait seam is wired;
/// per-EgressKind assertions belong in e7–e16.
struct AllowEgress;

#[async_trait]
impl EgressGate for AllowEgress {
    async fn check(&self, _kind: &EgressKind, _payload: &str) -> EgressDecision {
        EgressDecision::Allow
    }
}

#[tokio::test]
async fn run_with_tools_and_hooks_noop_matches_run_with_tools() {
    let prompt = "hello W6.1";

    let baseline = run_with_tools(
        EchoResponder::new(),
        NoopToolExecutor,
        Vec::<ToolDefinition>::new(),
        "system",
        prompt,
    )
    .await
    .expect("baseline run_with_tools must succeed");

    let hooked = run_with_tools_and_hooks(
        EchoResponder::new(),
        NoopToolExecutor,
        Vec::<ToolDefinition>::new(),
        HookBundle::noop(),
        "system",
        prompt,
    )
    .await
    .expect("run_with_tools_and_hooks with noop hooks must succeed");

    assert_eq!(
        baseline, hooked,
        "noop HookBundle must produce a verbatim reply equivalent to run_with_tools"
    );
}

#[tokio::test]
async fn run_with_tools_and_hooks_accepts_custom_egress_gate() {
    // HookBundle has no builder today, so we construct it directly via
    // its public fields — exactly how downstream callers will assemble
    // bundles. Only the egress slot is swapped; everything else stays at
    // the noop default.
    let hooks = HookBundle {
        egress: Arc::new(AllowEgress),
        sandbox: Arc::new(NoopSandboxExecutor),
        secrets: Arc::new(InMemorySecrets::new()),
        approval: Arc::new(AutoApproveGate),
    };

    let reply = run_with_tools_and_hooks(
        EchoResponder::new(),
        NoopToolExecutor,
        Vec::<ToolDefinition>::new(),
        hooks,
        "system",
        "wire-up",
    )
    .await
    .expect("custom EgressGate seam must compile and run");

    assert!(
        reply.contains("wire-up"),
        "expected echo of the prompt, got: {reply:?}"
    );
}
