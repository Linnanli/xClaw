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
//! ## Crate status (Phase 1.1.0)
//!
//! This is the **skeleton-only** revision. It exposes:
//!
//! - The sandbox policy types (`types::SandboxPolicy`, `types::NetworkAccess`,
//!   `types::WritableRoot`) so callers across the workspace can construct
//!   policies on every platform.
//! - On non-Windows targets, [`unsupported`] returns a typed error indicating
//!   that the Windows sandbox is unavailable on the current platform.
//!
//! The actual Win32 sandbox logic (AppContainer, JobObject, network ACLs)
//! lands in subsequent PRs under tracker issue #250.

pub mod absolute_path;
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
