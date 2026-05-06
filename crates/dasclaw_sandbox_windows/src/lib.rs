// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/windows-sandbox-rs/src/lib.rs
// SPDX-License-Identifier: Apache-2.0

//! Windows-specific sandbox runtime for x-claw.
//!
//! Ported (mechanically, with import rewrites) from
//! `openai/codex` `codex-rs/windows-sandbox-rs/` at commit `6e838a19fa`.
//! See `vendor/codex-windows-sandbox/README.md` for the reference snapshot
//! and the porting plan in tracker issue #250.
//!
//! ## Crate status (Phase 1.1.4c)
//!
//! V'-b "port-on-demand" subset. The cross-platform PTY/process plumbing
//! (`pty::process`) and the `cfg(windows)`-only ConPTY backend
//! (`pty::win::*`, `RawConPty`) are in tree, plus the leaf path-key,
//! sandbox setup, and logging utilities consumed across the upstream crate.
//!
//! Currently exposed:
//!
//! - Sandbox policy types (`types::SandboxPolicy`, `types::NetworkAccess`,
//!   `types::WritableRoot`).
//! - String / path utilities (`string_util`, `absolute_path`,
//!   `path_normalization`).
//! - Sandbox setup helpers (`sandbox_utils::ensure_codex_home_exists`,
//!   `sandbox_utils::inject_git_safe_directory`).
//! - Sandbox audit log (`logging::log_start` / `log_success` / `log_failure`
//!   / `log_note` / `debug_log`).
//! - DPAPI wrappers (`dpapi::protect` / `dpapi::unprotect`, `cfg(windows)`).
//! - Cross-platform PTY/process driver adapter (`pty::ProcessDriver`,
//!   `pty::SpawnedProcess`, `pty::TerminalSize`, `pty::spawn_from_driver`).
//! - On non-Windows targets, [`unsupported`] returns a typed error indicating
//!   that the Windows sandbox runtime is unavailable.
//!
//! The actual Win32 sandbox logic (AppContainer, JobObject, network ACLs)
//! lands in subsequent PRs under tracker issue #263 / #250.

pub mod absolute_path;
#[cfg(windows)]
pub mod dpapi;
pub mod logging;
pub mod path_normalization;
pub mod pty;
pub mod sandbox_utils;
pub mod string_util;
pub mod types;

/// Returned by sandbox entry points on platforms where the Windows sandbox is
/// unavailable.
///
/// Until the Win32 implementation lands (tracker #250), this is the only
/// surface the rest of the workspace can call into.
pub fn unsupported() -> anyhow::Error {
    anyhow::anyhow!(
        "dasclaw_sandbox_windows: Windows sandbox runtime is not yet implemented on this build (Phase 1.1.0 skeleton)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_message_is_stable() {
        let err = unsupported();
        let msg = err.to_string();
        assert!(msg.contains("dasclaw_sandbox_windows"));
        assert!(msg.contains("Phase 1.1.0"));
    }
}
