// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/windows-sandbox-rs/src/bin/setup_main.rs
// SPDX-License-Identifier: Apache-2.0
//
// Verbatim port. Mechanical edits:
//   * panic message: "codex-windows-sandbox-setup is Windows-only"
//     -> "dasclaw-sandbox-setup is Windows-only" (binary rename per
//     ADR-130 §2).

#[path = "../setup_main_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("dasclaw-sandbox-setup is Windows-only"); // safety: verbatim from codex 6e838a19fa setup_main.rs (binary refuses to run on non-Windows targets)
}
