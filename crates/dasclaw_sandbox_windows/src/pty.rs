// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/utils/pty/src/lib.rs
// SPDX-License-Identifier: Apache-2.0

//! PTY/process plumbing required by the Windows sandbox.
//!
//! V'-b "port-on-demand" subset of `codex-utils-pty`. Only the symbols
//! actually consumed by `windows-sandbox-rs` are exposed:
//!
//! - [`TerminalSize`] — terminal cell dimensions for spawn / resize.
//! - [`SpawnedProcess`] — return value of spawn helpers.
//! - [`ProcessDriver`] — adapter for backends that own their own transport.
//! - [`spawn_from_driver`] — turns a [`ProcessDriver`] into a [`SpawnedProcess`].
//! - `RawConPty` — `cfg(windows)` only, lands with PR-1.1.3b.
//!
//! Upstream's `pipe`, `pty`, `process_group`, and bundled `tests` modules
//! are intentionally omitted: nothing in `windows-sandbox-rs` imports them.

mod process;

pub use process::ProcessDriver;
pub use process::ProcessHandle;
pub use process::SpawnedProcess;
pub use process::TerminalSize;
pub use process::combine_output_receivers;
pub use process::spawn_from_driver;
