// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/windows-sandbox-rs/src/bin/command_runner.rs
// SPDX-License-Identifier: Apache-2.0
//
// Verbatim port. Mechanical edits:
//   * panic message: "codex-command-runner is Windows-only"
//     -> "dasclaw-command-runner is Windows-only" (binary rename per
//     ADR-130 §2).

#[path = "../elevated/command_runner_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("dasclaw-command-runner is Windows-only"); // safety: verbatim from codex 6e838a19fa command_runner.rs (binary refuses to run on non-Windows targets)
}
