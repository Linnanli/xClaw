//! Headless `dasclaw-cli sandbox-exec` subcommand backend
//! (ADR-153 §1.1 A6 / e12 — binary-level slice).
//!
//! Runs a single `bash -c <command>` through
//! [`dasclaw_exec::SandboxedExecutor`] under
//! [`SandboxPolicy::new_read_only_policy`] — the headless CLI default
//! that denies writes and turns off network access. The shipped
//! `dasclaw-cli` binary uses this entry to let CI assert the
//! deny-by-default contract end-to-end (exit code, stderr) without an
//! LLM key, complementing the library-seam tool-loop test in
//! [`tests/safety_sandbox_default_deny_e2e.rs`](../../tests/safety_sandbox_default_deny_e2e.rs).
//!
//! ## Contract
//!
//! - Returns `Ok(stdout_as_string)` when the sandbox admits the command
//!   *and* it exits zero. The caller is expected to forward `stdout`
//!   verbatim to its own stdout.
//! - Returns `Err(anyhow::Error)` for every failure mode (sandbox
//!   denial, non-zero exit, helper missing). The error chain is
//!   composed so that the rendered message *always* contains the
//!   literal token `sandbox`, which the binary-level e2e
//!   ([`tests/cli_sandbox_default_deny_e2e.rs`](../../tests/cli_sandbox_default_deny_e2e.rs))
//!   pins. The `main` wrapper renders the error to stderr and exits
//!   non-zero.
//!
//! ## Scope
//!
//! - macOS only is the platform actually enforced by the kernel today
//!   (Seatbelt backend, ADR-135 §3 PR-C1). Linux + Windows helpers are
//!   tracked separately and **not** loaded here — on those platforms the
//!   backend fails closed with a `sandbox` error, which is the safe
//!   default for a deny-by-default contract.
//! - Only the `ReadOnly` policy is wired in this slice. `WorkspaceWrite`
//!   plus `.git` / `.codex` / `.dasclaw` carve-outs land in #870.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
use dasclaw_sandbox::SandboxablePreference;
use dasclaw_workspace_cap::policy::SandboxPolicy;

/// Run `bash -c <command>` under the headless CLI's default-deny
/// `ReadOnly` sandbox policy in `cwd`.
///
/// On success returns the captured stdout. On sandbox denial / non-zero
/// exit returns an `anyhow::Error` whose rendered message starts with
/// `sandbox` so callers (and the binary-level e2e) can rely on a stable
/// stderr token.
pub async fn run_sandbox_exec(command: &str, cwd: PathBuf) -> Result<String> {
    if command.trim().is_empty() {
        return Err(anyhow!("sandbox: empty --command"));
    }

    // SandboxedExecutor::execute is synchronous and spawns
    // `sandbox-exec` (macOS) / a helper child (linux/windows). Hop to
    // the blocking pool so we don't stall the tokio runtime when the
    // host kernel is slow.
    let command_str = command.to_owned();
    let cwd_for_exec = cwd.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let executor = SandboxedExecutor::new(
            SandboxPolicy::new_read_only_policy(),
            SandboxablePreference::Require,
            false,
            None,
        );
        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c").arg(&command_str);
        executor.execute(ExecRequest {
            command: cmd,
            cwd: cwd_for_exec,
        })
    })
    .await
    .context("sandbox: blocking pool join failed")?;

    match outcome {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!(
                "sandbox: command exited with status {}: {}",
                output.status,
                stderr.trim()
            ))
        }
        Err(err) => Err(anyhow!("sandbox: denied: {err}")),
    }
}
