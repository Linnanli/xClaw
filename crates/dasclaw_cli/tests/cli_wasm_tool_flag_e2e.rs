//! #867 — `--wasm-tool` CLI flag end-to-end test (ADR-153 §1.1 A7).
//!
//! Exercises the production wiring added in #867: `ToolArgs::wasm_tool`
//! → `dasclaw_cli::wasm::load_wasm_tool` → composite assembly →
//! `run_with_tools_and_hooks` agent loop. Asserts the default-deny
//! capability posture survives the prod path: a wasm tool that calls
//! `host.http-request` without an `http` grant surfaces as
//! `is_error=true` with the host gate message intact, exactly like the
//! W6.6c library-seam e13 test.
//!
//! ## Why a separate file (not folded into `safety_wasm_capability_optin_e2e.rs`)
//!
//! The W6.6c test fixes the library seam (`ToolExecutor → WasmToolWrapper`).
//! This file fixes the *prod CLI helper* `dasclaw_cli::wasm::load_wasm_tool`:
//! the file-stem → tool-name derivation, the file-read error path, and the
//! capability posture chosen by the CLI (`Capabilities::default()` rather
//! than caller-supplied). Keeping the two tests separate means a refactor
//! of either surface fails the right test.
//!
//! ## Non-goals (#867 §"非目标")
//!
//! - Does **not** spawn the `dasclaw-cli` binary (assert_cmd would be a
//!   new pattern in the repo — tracked as #868).
//! - Does **not** test capability *grants* (#869 covers the broader
//!   matrix: http allowed, workspace writes, secrets injection).
//! - Does **not** test wasm tools alongside MCP / builtin tools — the
//!   composite path is already covered by the upstream A5/A7 tests.

#![cfg(feature = "wasm-tools")]

use std::io::Write;

use dasclaw_cli::run_with_tools_and_hooks;
use dasclaw_cli::wasm::load_wasm_tool;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::ToolCall;
use dasclaw_runtime::ToolExecutor;
use dasclaw_wasm_tools::NO_HTTP_CAP_WASM;
use serde_json::json;
use tempfile::NamedTempFile;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;
use fixtures::{ScriptedResponder, text_turn, tool_call_turn};

/// Write the bundled `NO_HTTP_CAP_WASM` component to a tempfile so
/// `load_wasm_tool` exercises its real path-based loader (file read +
/// stem-based name derivation). The `.wasm` suffix matters: `load_wasm_tool`
/// derives the tool name from the file stem.
fn fixture_wasm_on_disk(name: &str) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .prefix(name)
        .suffix(".wasm")
        .tempfile()
        .expect("create wasm tempfile");
    file.write_all(NO_HTTP_CAP_WASM)
        .expect("write wasm bytes to tempfile");
    file.flush().expect("flush tempfile");
    file
}

/// Drives the same scripted dialogue as W6.6c e13 but through the
/// production CLI loader. The model emits a single tool call into the
/// disk-loaded wasm component; the capability gate refuses it; the
/// agent loop receives a reported tool error and completes with the
/// acknowledgement turn.
#[tokio::test]
async fn req_dasclaw_cli_867_wasm_tool_flag_default_deny_surfaces_as_tool_error() {
    let wasm_file = fixture_wasm_on_disk("fixture_http_");
    let loaded = load_wasm_tool(wasm_file.path())
        .await
        .expect("load_wasm_tool should accept a valid wasip2 component");

    // Tool name MUST be the file stem so the scripted tool call resolves.
    let expected_name = wasm_file
        .path()
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("tempfile has a stem")
        .to_string();
    assert_eq!(
        loaded.definition.name, expected_name,
        "load_wasm_tool must derive the LLM-facing tool name from the file stem"
    );

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(&expected_name, "call-1", json!({})),
        text_turn("acknowledged: capability gate refused"),
    ]);

    let reply = run_with_tools_and_hooks(
        responder,
        loaded.executor,
        vec![loaded.definition],
        HookBundle::noop(),
        "you are running inside a capability-gated CLI",
        "please call the wasm fixture tool",
    )
    .await
    .expect("agent loop must not crash on capability denial");

    assert_eq!(reply, "acknowledged: capability gate refused");
}

/// Pins the exact tool-result envelope: `is_error=true` plus the host
/// gate string `"HTTP capability not granted"`. Drives the executor
/// directly (bypassing the agent loop) so the assertion isolates the
/// CLI's `Capabilities::default()` choice from the broader scripted
/// dialogue.
#[tokio::test]
async fn req_dasclaw_cli_867_wasm_tool_flag_tool_result_carries_host_gate_message() {
    let wasm_file = fixture_wasm_on_disk("fixture_http_");
    let loaded = load_wasm_tool(wasm_file.path())
        .await
        .expect("load_wasm_tool should accept a valid wasip2 component");

    let call = ToolCall {
        id: "direct-call".to_string(),
        name: loaded.definition.name.clone(),
        arguments: json!({}),
        reasoning: None,
    };

    let result = loaded
        .executor
        .execute(&call)
        .await
        .expect("ToolExecutor must not raise HostError on capability denial");

    assert!(
        result.is_error,
        "capability-gated wasm tool must report is_error=true, got: {result:?}"
    );
    assert!(
        result.content.contains("HTTP capability not granted"),
        "tool result content must carry the host gate string verbatim, got: {}",
        result.content
    );
}

/// Failure-path coverage: a bogus path must surface as a normal error
/// (no panic, no `HostError`) so `main.rs` can attach `with_context`
/// without surprise.
#[tokio::test]
async fn req_dasclaw_cli_867_wasm_tool_flag_missing_file_surfaces_as_error() {
    let missing = std::env::temp_dir().join("dasclaw_867_does_not_exist.wasm");
    // Defensive: the file genuinely should not exist.
    let _ = std::fs::remove_file(&missing);

    let outcome = load_wasm_tool(&missing).await;
    let err = outcome
        .err()
        .expect("load_wasm_tool must error on missing file");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("reading wasm component"),
        "error must mention the read step for diagnosability, got: {msg}"
    );
}
