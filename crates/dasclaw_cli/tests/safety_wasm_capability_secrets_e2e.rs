//! ADR-153 §1.1 A7 follow-up — capability matrix: `secret_exists`
//! default-deny e2e (issue #869).
//!
//! Companion to the http and workspace rows. Drives the wasip2 fixture
//! through the `action="secrets"` branch, which calls the host
//! `secret-exists` import. With `Capabilities::default()` (no `secrets`
//! granted), the host returns `false` over the `bool` WIT signature.
//! There is no error channel back to the guest at this WIT signature, so
//! the default-deny gate is observable only as `false`. The fixture
//! turns that into a stable error prefix so the tool-result envelope
//! pins both contracts.

use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::ToolCall;
use dasclaw_wasm_tools::Capabilities;
use serde_json::json;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod agent_loop_fixtures;
#[path = "fixtures/wasm_capability_helpers.rs"]
mod wasm_capability_helpers;

use agent_loop_fixtures::{ScriptedResponder, text_turn, tool_call_turn};
use wasm_capability_helpers::{CapWasmExecutor, fixture_tool_def};

#[tokio::test]
async fn req_dasclaw_cli_safety_a7_wasm_no_secrets_capability_surfaces_as_tool_error() {
    let tool_name = "fixture_secrets";
    let executor = CapWasmExecutor::build(tool_name, Capabilities::default(), None).await;

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(tool_name, "call-sec", json!({ "action": "secrets" })),
        text_turn("acknowledged: secrets capability denied"),
    ]);

    let outcome = run_with_tools_and_hooks(
        responder,
        executor,
        vec![fixture_tool_def(tool_name)],
        HookBundle::noop(),
        "you are running inside a capability-gated CLI",
        "please call the fixture secrets tool",
    )
    .await;

    let reply = outcome.expect("agent loop must not crash on secrets capability denial");
    assert_eq!(reply, "acknowledged: secrets capability denied");
}

#[tokio::test]
async fn req_dasclaw_cli_safety_a7_wasm_secrets_tool_result_carries_denial_signal() {
    let tool_name = "fixture_secrets";
    let executor = CapWasmExecutor::build(tool_name, Capabilities::default(), None).await;

    let call = ToolCall {
        id: "direct-call-sec".to_string(),
        name: tool_name.to_string(),
        arguments: json!({ "action": "secrets" }),
        reasoning: None,
    };

    let result = dasclaw_runtime::ToolExecutor::execute(&executor, &call)
        .await
        .expect("ToolExecutor must not raise HostError on capability denial");

    assert!(
        result.is_error,
        "capability-gated secret-exists must report is_error=true, got: {result:?}"
    );
    assert!(
        result.content.contains("secret-exists returned false"),
        "tool result content must signal the default-deny gate, got: {}",
        result.content
    );
    // Negative assertion: the envelope must not claim the probe succeeded.
    assert!(
        !result.content.contains("\"secret_exists\":true"),
        "no secret_exists=true payload may appear when capability is denied, got: {}",
        result.content
    );
}
