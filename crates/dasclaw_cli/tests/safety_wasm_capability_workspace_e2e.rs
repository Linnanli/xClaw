//! ADR-153 §1.1 A7 follow-up — capability matrix: `workspace_read`
//! default-deny e2e (issue #869).
//!
//! Companion to `safety_wasm_capability_optin_e2e.rs` (the http row).
//! Drives the same wasip2 fixture through a fresh `action="workspace"`
//! branch that calls the host `workspace-read` import. With
//! `Capabilities::default()` (no `workspace_read` granted), the host
//! returns `None` over the `option<string>` WIT signature; the fixture
//! reports that observation as an error so the tool-result envelope
//! preserves the "no data crossed the boundary" contract.
//!
//! ## Why we assert on the fixture-emitted string and not a host string
//!
//! The host `workspace-read` export returns `option<string>` — there is
//! no error channel back to the guest, so the default-deny gate is
//! "the option is `None`". The fixture turns that into a stable error
//! prefix (`"workspace-read returned None"`) so this test pins both the
//! WIT-level contract (option is empty) and the agent-loop contract
//! (`is_error=true`).

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
async fn req_dasclaw_cli_safety_a7_wasm_no_workspace_capability_surfaces_as_tool_error() {
    let tool_name = "fixture_workspace";
    let executor = CapWasmExecutor::build(tool_name, Capabilities::default(), None).await;

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(tool_name, "call-ws", json!({ "action": "workspace" })),
        text_turn("acknowledged: workspace capability denied"),
    ]);

    let outcome = run_with_tools_and_hooks(
        responder,
        executor,
        vec![fixture_tool_def(tool_name)],
        HookBundle::noop(),
        "you are running inside a capability-gated CLI",
        "please call the fixture workspace tool",
    )
    .await;

    let reply = outcome.expect("agent loop must not crash on workspace capability denial");
    assert_eq!(reply, "acknowledged: workspace capability denied");
}

#[tokio::test]
async fn req_dasclaw_cli_safety_a7_wasm_workspace_tool_result_carries_denial_signal() {
    let tool_name = "fixture_workspace";
    let executor = CapWasmExecutor::build(tool_name, Capabilities::default(), None).await;

    let call = ToolCall {
        id: "direct-call-ws".to_string(),
        name: tool_name.to_string(),
        arguments: json!({ "action": "workspace" }),
        reasoning: None,
    };

    let result = dasclaw_runtime::ToolExecutor::execute(&executor, &call)
        .await
        .expect("ToolExecutor must not raise HostError on capability denial");

    assert!(
        result.is_error,
        "capability-gated workspace-read must report is_error=true, got: {result:?}"
    );
    assert!(
        result.content.contains("workspace-read returned None"),
        "tool result content must signal the default-deny gate, got: {}",
        result.content
    );
    // Negative assertion: no file content from the host crossed the
    // boundary. The fixture never has access to disk in this config, so
    // the only way `test.txt` could appear in the envelope is a host
    // bug that leaks the path's contents.
    assert!(
        !result.content.contains("\"workspace_read\":"),
        "no workspace_read payload may appear when capability is denied, got: {}",
        result.content
    );
}
