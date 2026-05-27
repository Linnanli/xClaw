//! W6.6 — ADR-153 §1.1 A6b: L2 sub-process sandbox **WorkspaceWrite**
//! policy contract.
//!
//! Sister to [`safety_sandbox_default_deny_e2e.rs`] (which pins the
//! `ReadOnly` row of the matrix). This file pins the production-default
//! `WorkspaceWrite` row: writes **inside** the workspace root (cwd) are
//! admitted, writes **outside** must be refused by the OS sandbox and
//! surface as `is_error=true` with a `sandbox` reason.
//!
//! ## Platform gate
//!
//! macOS only: the Seatbelt backend in `dasclaw_sandbox::macos` is fully
//! wired (W2.2 / ADR-135 §3 PR-C1) and `/usr/bin/sandbox-exec` ships in
//! the base image, so the OS kernel actually enforces the policy locally
//! and in CI macOS runners. Linux landlock + Windows backends need
//! external helpers and are tracked separately by #481.
//!
//! ## Non-goals
//!
//! - Not asserting binary-level CLI exit codes (that is `assert_cmd`
//!   territory; see [`cli_sandbox_default_deny_e2e.rs`] for the A6
//!   binary-seam contract).
//! - Not exercising the writable-root hole-in-hole carve-out logic
//!   (`.git` / `.dasclaw` / `.codex`). That is A6c, lives in
//!   [`safety_sandbox_carve_out_e2e.rs`].

#![cfg(target_os = "macos")]

use dasclaw_core::hooks::HookBundle;
use dasclaw_workspace_cap::policy::SandboxPolicy;
use serde_json::json;

#[path = "fixtures/sandbox_bash_executor.rs"]
mod sandbox_bash_executor;
use sandbox_bash_executor::{
    SandboxedBashExecutor, ScriptedResponder, ack_turn, bash_tool_def, text_turn, tool_call_turn,
};

/// Build a `WorkspaceWrite` policy whose only writable root is `cwd`.
///
/// The factory `new_workspace_write_policy()` also auto-promotes
/// `/tmp` and `$TMPDIR` to writable roots. On macOS `tempfile::tempdir`
/// lives under `$TMPDIR` (`/var/folders/...`), so leaving those flags
/// at their defaults would let writes to *any* tempdir slip through and
/// mask the actual cwd-bounded contract this row is meant to pin.
fn cwd_only_workspace_write() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

/// A6b positive path — a write **inside** the workspace root (here
/// `cwd`, which `new_workspace_write_policy()` auto-promotes to a
/// writable root) must succeed under the same kernel-enforced sandbox
/// that denies the corresponding `ReadOnly` case.
///
/// This is the row that proves WorkspaceWrite is not just "ReadOnly with
/// a typo": without it the deny test below could trivially pass from any
/// sandbox misconfiguration that denies every write.
#[tokio::test]
async fn req_dasclaw_cli_loop_a6b_workspace_write_admits_write_under_cwd() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let target = tempdir.path().join("a6b_inside_cwd.txt");
    let target_path_str = target.to_string_lossy().into_owned();

    let executor =
        SandboxedBashExecutor::new(cwd_only_workspace_write(), tempdir.path().to_path_buf());

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-write-inside-1",
            json!({ "command": format!("echo allowed-by-sandbox > {target_path_str}") }),
        ),
        text_turn("done"),
    ]);

    let reply = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please write a marker inside the workspace",
    )
    .await
    .expect("agent loop ran to completion");

    assert_eq!(reply, "done");
    assert!(
        target.exists(),
        "WorkspaceWrite must admit writes under cwd; file missing at {target:?}"
    );
    let body = std::fs::read_to_string(&target).expect("read marker file");
    assert!(
        body.contains("allowed-by-sandbox"),
        "marker file content unexpected: {body:?}"
    );
}

/// A6b deny path — a write **outside** the workspace root must be
/// refused by the OS sandbox. The agent loop must still complete cleanly
/// (the model gets to see the tool error and emit an acknowledgement),
/// matching the same fail-safe contract as A6 / e12.
#[tokio::test]
async fn req_dasclaw_cli_loop_a6b_workspace_write_denies_write_outside_cwd() {
    let tempdir = tempfile::tempdir().expect("create tempdir cwd");
    let outside_dir = tempfile::tempdir().expect("create tempdir outside");
    let target = outside_dir.path().join("a6b_outside_cwd.txt");
    let target_path_str = target.to_string_lossy().into_owned();

    assert!(
        !target.exists(),
        "precondition: outside-cwd target must not exist yet"
    );

    let executor =
        SandboxedBashExecutor::new(cwd_only_workspace_write(), tempdir.path().to_path_buf());

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-write-outside-1",
            json!({ "command": format!("echo blocked-by-sandbox > {target_path_str}") }),
        ),
        ack_turn(),
    ]);

    let outcome = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please attempt to write outside the workspace",
    )
    .await;

    assert!(
        outcome.is_ok(),
        "agent loop must not crash on a sandbox denial, got {outcome:?}"
    );
    let reply = outcome.expect("loop completion checked above");
    assert!(
        reply.contains("sandbox") || reply.contains("understood"),
        "reply should reflect the denial, got {reply:?}"
    );
    assert!(
        !target.exists(),
        "WorkspaceWrite must deny writes outside cwd; file was created at {target:?}"
    );
}
