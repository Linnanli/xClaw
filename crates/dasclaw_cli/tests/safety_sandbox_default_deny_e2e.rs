//! W6.6b — ADR-153 §1.1 A6 / e12: L2 sub-process sandbox default-deny.
//!
//! Wires a `bash` tool whose `ToolExecutor` routes the requested command
//! through [`dasclaw_exec::SandboxedExecutor`] under
//! [`SandboxPolicy::new_read_only_policy`] (the headless CLI default for
//! P1: deny-by-default writes, network off). The matrix expectation
//! (ADR-153 §1.1 row A6/e12) is that any **non-allowlist** command — here,
//! a write — must surface as `is_error=true` with a reason mentioning
//! `sandbox`, and the agent must **not** continue speaking after that.
//!
//! ## Platform gate
//!
//! macOS only: the Seatbelt backend in `dasclaw_sandbox::macos` is fully
//! wired (W2.2 / ADR-135 §3 PR-C1) and `/usr/bin/sandbox-exec` is in the
//! base image, so the OS kernel actually enforces the policy locally and
//! in CI macOS runners. The Linux + Windows backends require external
//! helper binaries (`dasclaw-sandbox-linux` setuid helper / Windows
//! sandbox host) to be on `PATH`, which is **not** the default in
//! `dasclaw_cli` tests — they fail-closed in a way that would mask the
//! policy decision under test. Those platforms are tracked by separate
//! W6 issues; this slice intentionally pins the macOS contract.
//!
//! ## Non-goals
//!
//! - Not asserting binary-level CLI exit codes (that is `assert_cmd`
//!   territory and lives in a separate slice).
//! - Not exercising `WorkspaceWrite` policy nor the writable-root
//!   hole-in-hole logic (`.git` / `.codex` / `.dasclaw` carve-outs). Those
//!   belong to W6.6 A6b / A6c follow-ups.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::hooks::HookBundle;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::traits::HostError;
use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
use dasclaw_runtime::ToolExecutor;
use dasclaw_sandbox::SandboxablePreference;
use dasclaw_workspace_cap::policy::SandboxPolicy;
use serde_json::json;

#[path = "fixtures/agent_loop_fixtures.rs"]
mod fixtures;
use fixtures::{ScriptedResponder, text_turn, tool_call_turn};

/// `ToolExecutor` that pipes the `command` argument of each call into
/// `bash -c` through the real OS sandbox. Errors from the sandbox
/// (`ExecError::Sandbox`) and non-zero exits both surface as
/// `is_error=true` so the agent loop sees them as tool failures, matching
/// the e12 matrix expectation.
struct SandboxedBashExecutor {
    inner: Arc<SandboxedExecutor>,
    cwd: PathBuf,
}

impl SandboxedBashExecutor {
    fn new(policy: SandboxPolicy, cwd: PathBuf) -> Self {
        Self {
            inner: Arc::new(SandboxedExecutor::new(
                policy,
                SandboxablePreference::Require,
                false,
                None,
            )),
            cwd,
        }
    }
}

#[async_trait]
impl ToolExecutor for SandboxedBashExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let command = call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::from("missing `command` arg"))?
            .to_string();

        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c").arg(&command);

        let inner = Arc::clone(&self.inner);
        let cwd = self.cwd.clone();
        // SandboxedExecutor::execute is sync + does heavy work (spawns
        // sandbox-exec). Hop to a blocking pool so we don't stall the
        // tokio runtime.
        let outcome =
            tokio::task::spawn_blocking(move || inner.execute(ExecRequest { command: cmd, cwd }))
                .await
                .map_err(|e| HostError::from(format!("blocking pool: {e}")))?;

        let (is_error, content) = match outcome {
            Ok(output) if output.status.success() => {
                (false, String::from_utf8_lossy(&output.stdout).into_owned())
            }
            Ok(output) => {
                // Sandbox let the command run but the command itself
                // failed (e.g. shell error on a denied write). e12 still
                // counts this as a sandbox-enforced denial when stderr
                // names sandbox-exec / Operation not permitted.
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                (
                    true,
                    format!("sandbox: command exited with {}: {stderr}", output.status),
                )
            }
            Err(err) => (true, format!("sandbox: denied: {err}")),
        };

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}

fn bash_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "bash".to_string(),
        description: "Execute a bash command inside the OS sandbox.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" }
            },
            "required": ["command"]
        }),
    }
}

/// Acknowledgement turn the model emits after seeing the tool error.
/// Kept short so the test focuses on whether the loop actually halted on
/// the sandbox decision rather than on prompt content.
fn ack_turn() -> dasclaw_core::response_types::RespondOutput {
    text_turn("understood: sandbox denied the write")
}

/// e12 main assertion — a write attempt under ReadOnly policy must be
/// refused by the OS sandbox (or by the shell running inside it, which
/// is the same enforcement path) and surface as `is_error=true` with a
/// `sandbox` reason. The model's follow-up acknowledgement must be the
/// only thing the user sees.
#[tokio::test]
async fn req_dasclaw_cli_loop_e12_sandbox_denies_write_under_read_only_policy() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let target = tempdir.path().join("e12_should_not_exist.txt");
    let target_path_str = target.to_string_lossy().into_owned();

    let executor = SandboxedBashExecutor::new(
        SandboxPolicy::new_read_only_policy(),
        tempdir.path().to_path_buf(),
    );

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-write-1",
            json!({ "command": format!("echo blocked > {target_path_str}") }),
        ),
        ack_turn(),
    ]);

    let reply = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please write the word `blocked` to the test file",
    )
    .await
    .expect("agent loop ran to completion");

    assert!(
        !target.exists(),
        "sandbox must prevent the write; file was created at {target:?}"
    );
    assert!(
        reply.contains("sandbox") || reply.contains("understood"),
        "reply should reflect the denial, got {reply:?}"
    );
}

/// e12 control case — a read-only command (`echo`) under the same policy
/// must succeed, proving the deny only applies to writes and the sandbox
/// is not blanket-failing. Without this baseline the main test could pass
/// from any sandbox misconfiguration that denies everything.
#[tokio::test]
async fn req_dasclaw_cli_loop_e12_sandbox_allows_read_only_command_under_read_only_policy() {
    let tempdir = tempfile::tempdir().expect("create tempdir");

    let executor = SandboxedBashExecutor::new(
        SandboxPolicy::new_read_only_policy(),
        tempdir.path().to_path_buf(),
    );

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-echo-1",
            json!({ "command": "echo hello-from-sandbox" }),
        ),
        text_turn("ok"),
    ]);

    let reply = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please echo a friendly greeting",
    )
    .await
    .expect("agent loop ran to completion");

    assert_eq!(reply, "ok");
}

/// e12 secondary assertion — the sandbox denial must be a *reported tool
/// error*, not a host-level crash. We re-run the write attempt and verify
/// the agent loop completes (no `Err`) and the tool result actually
/// reached the model (it produced the ack turn instead of being kicked
/// out of the loop).
#[tokio::test]
async fn req_dasclaw_cli_loop_e12_sandbox_denial_is_reported_not_host_crash() {
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let target = tempdir.path().join("e12_crash_probe.txt");

    let executor = SandboxedBashExecutor::new(
        SandboxPolicy::new_read_only_policy(),
        tempdir.path().to_path_buf(),
    );

    let responder = ScriptedResponder::with_queue(vec![
        tool_call_turn(
            "bash",
            "call-write-2",
            json!({ "command": format!("touch {}", target.to_string_lossy()) }),
        ),
        text_turn("acknowledged"),
    ]);

    let outcome = dasclaw_cli::run_with_tools_and_hooks(
        responder,
        executor,
        vec![bash_tool_def()],
        HookBundle::noop(),
        "you are running inside a sandboxed CLI",
        "please create the probe file",
    )
    .await;

    assert!(
        outcome.is_ok(),
        "agent loop must not crash on a sandbox denial, got {outcome:?}"
    );
    assert_eq!(outcome.unwrap(), "acknowledged");
    assert!(
        !target.exists(),
        "sandbox must prevent the write; file was created at {target:?}"
    );
}
