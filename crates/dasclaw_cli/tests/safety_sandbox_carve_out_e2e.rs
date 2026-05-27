//! W6.6 — ADR-153 §1.1 A6c: L2 sub-process sandbox **hole-in-hole**
//! ("洞中洞") carve-out contract.
//!
//! Under `WorkspaceWrite`, the kernel-enforced sandbox profile must
//! additionally refuse writes to sensitive sub-trees of the workspace
//! root — `.git/` and `.codex/` (when present) — even though the rest
//! of the cwd is writable. Without this carve-out an agent could
//! rewrite git hooks to land RCE on the next commit
//! ([`dasclaw_workspace_cap::policy`] docs §"洞中洞").
//!
//! ## Platform gate
//!
//! macOS only, mirroring [`safety_sandbox_workspace_write_e2e.rs`].
//! The carve-out is materialised via Seatbelt's exclusion clauses by
//! `dasclaw_sandboxing::seatbelt`, which projects the cwd-anchored
//! carve-out list emitted by
//! [`dasclaw_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd`]
//! (see `crates/dasclaw_protocol/src/permissions.rs` `default_read_only_subpaths_for_writable_root`).
//! Linux landlock / Windows DACL wiring is out of scope for this slice.
//!
//! ## Scope vs. policy decision layer
//!
//! The decision-layer policy in `dasclaw_workspace_cap` carves out
//! `.dasclaw/` (with `protect_missing_project_meta=true`), and as of
//! #874 the kernel projection in `dasclaw_protocol::permissions::
//! default_read_only_subpaths_for_writable_root` also emits `.dasclaw`
//! alongside `.git` and `.codex`. All three rows below are therefore
//! green; any future regression that lets `.dasclaw` writes through
//! must be treated as a contract break, not a known gap.
//!
//! ## Test rows
//!
//! 1. `.git/config` and `.codex/config` — pre-created on disk so the
//!    policy actually emits the carve-out (`.git` / `.codex` carves
//!    only trigger when the directory is present, mirroring upstream
//!    `codex` behaviour).
//! 2. `.dasclaw/state` — green post-#874. Pins the kernel-side carve-out
//!    for the dasclaw project-meta directory.
//! 3. Regular subdir control — proves the carve-out is targeted and
//!    not just "WorkspaceWrite denies everything".

#![cfg(target_os = "macos")]

use std::fs;

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
/// See the twin helper in `safety_sandbox_workspace_write_e2e.rs` for
/// the rationale: on macOS `tempfile::tempdir` lives under `$TMPDIR`,
/// and a `$TMPDIR` writable root only carries cwd-bound carve-outs when
/// `root == cwd`. Excluding both `/tmp` and `$TMPDIR` keeps cwd the
/// only writable root, so the `.git` / `.dasclaw` / `.codex` carve-outs
/// are the only thing standing between the agent and the sensitive file.
fn cwd_only_workspace_write() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

/// A6c deny path — writing into a pre-existing `.git/` or `.codex/`
/// carve-out must be refused even though the surrounding cwd is
/// writable.
#[tokio::test]
async fn req_dasclaw_cli_loop_a6c_carve_out_denies_dot_git_and_dot_codex() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let cwd = tempdir.path();
    let dot_git = cwd.join(".git");
    let dot_codex = cwd.join(".codex");
    fs::create_dir(&dot_git).expect("seed .git dir so policy emits carve-out");
    fs::create_dir(&dot_codex).expect("seed .codex dir so policy emits carve-out");

    let git_target = dot_git.join("config");
    let codex_target = dot_codex.join("config");
    let git_path_str = git_target.to_string_lossy().into_owned();
    let codex_path_str = codex_target.to_string_lossy().into_owned();

    let executor = SandboxedBashExecutor::new(cwd_only_workspace_write(), cwd.to_path_buf());

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-carve-git-1",
            json!({ "command": format!("echo pwn > {git_path_str}") }),
        ),
        tool_call_turn(
            "bash",
            "call-carve-codex-1",
            json!({ "command": format!("echo pwn > {codex_path_str}") }),
        ),
        ack_turn(),
    ]);

    let outcome = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please attempt to write inside .git and .codex",
    )
    .await;

    assert!(
        outcome.is_ok(),
        "agent loop must not crash on carve-out denial, got {outcome:?}"
    );
    let reply = outcome.expect("loop completion checked above");
    assert!(
        reply.contains("sandbox") || reply.contains("understood"),
        "reply should reflect the denial, got {reply:?}"
    );
    assert!(
        !git_target.exists(),
        ".git carve-out breached: file created at {git_target:?}"
    );
    assert!(
        !codex_target.exists(),
        ".codex carve-out breached: file created at {codex_target:?}"
    );
}

/// A6c — `.dasclaw/` is carved out alongside `.git` and `.codex` via
/// the kernel projection in
/// `dasclaw_protocol::permissions::default_read_only_subpaths_for_writable_root`
/// (extended in #874). A write into the carve-out must be refused by
/// seatbelt even though the surrounding cwd is writable.
#[tokio::test]
async fn req_dasclaw_cli_loop_a6c_carve_out_denies_dot_dasclaw() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let cwd = tempdir.path();
    let dot_dasclaw = cwd.join(".dasclaw");
    fs::create_dir(&dot_dasclaw).expect("seed .dasclaw dir so policy emits carve-out");

    let dasclaw_target = dot_dasclaw.join("state");
    let dasclaw_path_str = dasclaw_target.to_string_lossy().into_owned();

    let executor = SandboxedBashExecutor::new(cwd_only_workspace_write(), cwd.to_path_buf());

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-carve-dasclaw-1",
            json!({ "command": format!("echo pwn > {dasclaw_path_str}") }),
        ),
        ack_turn(),
    ]);

    let outcome = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please attempt to write inside .dasclaw",
    )
    .await;

    assert!(
        outcome.is_ok(),
        "agent loop must not crash on carve-out denial, got {outcome:?}"
    );
    assert!(
        !dasclaw_target.exists(),
        ".dasclaw carve-out breached: file created at {dasclaw_target:?}"
    );
}

/// A6c positive control — a write to a non-carved subdirectory of the
/// workspace must still succeed. This is the row that distinguishes
/// "carve-out is targeted" from "WorkspaceWrite is silently broken".
#[tokio::test]
async fn req_dasclaw_cli_loop_a6c_carve_out_admits_regular_subdir_write() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let cwd = tempdir.path();
    let target = cwd.join("regular_subdir").join("file.txt");
    let target_path_str = target.to_string_lossy().into_owned();

    let executor = SandboxedBashExecutor::new(cwd_only_workspace_write(), cwd.to_path_buf());

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-regular-subdir-1",
            json!({
                "command": format!(
                    "mkdir -p $(dirname {target_path_str}) && echo allowed > {target_path_str}"
                )
            }),
        ),
        text_turn("done"),
    ]);

    let reply = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please write into a regular subdir",
    )
    .await
    .expect("agent loop ran to completion");

    assert_eq!(reply, "done");
    assert!(
        target.exists(),
        "regular subdir write should be admitted; missing at {target:?}"
    );
    let body = fs::read_to_string(&target).expect("read regular subdir file");
    assert!(
        body.contains("allowed"),
        "regular subdir file content unexpected: {body:?}"
    );
}
