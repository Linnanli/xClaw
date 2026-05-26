//! Binary-level e2e for `dasclaw-cli sandbox-exec`
//! (ADR-153 §1.1 A6 / e12, follow-up to #868).
//!
//! Sister to the library-seam test
//! [`safety_sandbox_default_deny_e2e.rs`](safety_sandbox_default_deny_e2e.rs):
//! that one validates the agent loop, this one validates the **shipped
//! binary's** exit-code + stderr contract by driving the real `dasclaw-cli`
//! through [`assert_cmd::Command::cargo_bin`].
//!
//! ## Platform gate
//!
//! macOS only: the Seatbelt backend in `dasclaw_sandbox::macos` is fully
//! wired (W2.2 / ADR-135 §3 PR-C1) and `/usr/bin/sandbox-exec` is
//! available out of the box. Linux + Windows backends require helper
//! binaries that are not on `PATH` in the default test environment and
//! are tracked by separate W6 issues (#481 platform matrix).
//!
//! ## Contract pinned here
//!
//! 1. Sandbox denial → exit non-zero, stderr contains `sandbox`.
//! 2. Sandbox-admitted read-only command → exit zero, stdout carries
//!    the command's stdout verbatim.
//! 3. Empty `--command` → exit non-zero, stderr contains `sandbox`
//!    (fail-closed on degenerate input).
//!
//! Cases the library-seam test cannot observe (binary-level only):
//! - The agent loop never serialises an `ExitCode` to the OS; only the
//!   binary's outer `main` does.
//! - The agent loop renders sandbox errors into `ToolResult.content`;
//!   the binary renders them onto `stderr` via the `dasclaw: {err:#}`
//!   wrapper in `main`.

#![cfg(target_os = "macos")]

use assert_cmd::Command;
use predicates::prelude::*;

/// 1. Default-deny: a write under `ReadOnly` policy must surface as a
///    non-zero exit with `sandbox` in stderr, and the would-be target
///    file must not exist on disk.
#[test]
fn req_dasclaw_cli_bin_sandbox_exec_denies_write_under_read_only_policy() {
    let workdir = tempfile::tempdir().expect("create workdir");
    let target = workdir.path().join("sandbox_should_block.txt");
    assert!(
        !target.exists(),
        "precondition: target file should not exist yet"
    );

    let cmd_str = format!(
        "echo blocked-by-sandbox > {}",
        target
            .to_str()
            .expect("tempdir path must be valid UTF-8 on macOS test runners")
    );

    let mut cmd = Command::cargo_bin("dasclaw-cli").expect("locate dasclaw-cli binary");
    cmd.current_dir(workdir.path())
        .args(["sandbox-exec", "--command", &cmd_str]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("sandbox"));

    assert!(
        !target.exists(),
        "post-condition: ReadOnly sandbox must have prevented the write"
    );
}

/// 2. Sandbox-admitted positive path: a plain `echo` does not touch the
///    filesystem, so the `ReadOnly` policy admits it and the captured
///    stdout is forwarded verbatim with exit zero.
#[test]
fn req_dasclaw_cli_bin_sandbox_exec_admits_readonly_command() {
    let workdir = tempfile::tempdir().expect("create workdir");

    let mut cmd = Command::cargo_bin("dasclaw-cli").expect("locate dasclaw-cli binary");
    cmd.current_dir(workdir.path())
        .args(["sandbox-exec", "--command", "echo readonly-ok"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("readonly-ok"));
}

/// 3. Degenerate input: empty `--command` is rejected fail-closed with
///    a `sandbox` stderr token, matching the rest of the contract
///    rather than surfacing as a bare clap error or a silent zero-exit.
#[test]
fn req_dasclaw_cli_bin_sandbox_exec_rejects_empty_command() {
    let workdir = tempfile::tempdir().expect("create workdir");

    let mut cmd = Command::cargo_bin("dasclaw-cli").expect("locate dasclaw-cli binary");
    cmd.current_dir(workdir.path())
        .args(["sandbox-exec", "--command", "   "]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("sandbox"));
}
