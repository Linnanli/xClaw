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
//! ## Crate status (Phase 1.1.4h)
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
//! - Process/thread attribute list builder
//!   (`proc_thread_attr::ProcThreadAttributeList`, `cfg(windows)`).
//! - Setup error report types & redaction
//!   (`setup_error::SetupFailure`, `setup_error::SetupErrorCode`,
//!   `setup_error::write_setup_error_report`,
//!   `setup_error::read_setup_error_report`,
//!   `setup_error::sanitize_setup_metric_tag_value`).
//! - Per-workspace capability SID store (`cap::CapSids`,
//!   `cap::load_or_create_cap_sids`, `cap::workspace_cap_sid_for_cwd`).
//! - Sandbox environment scrubbers (`env::normalize_null_device_env`,
//!   `env::ensure_non_interactive_pager`, `env::inherit_path_env`,
//!   `env::apply_no_network_to_env`).
//! - Win32 utility helpers (`winutil::to_wide`, `winutil::format_last_error`,
//!   `winutil::resolve_sid`, `winutil::string_from_sid_bytes`,
//!   `winutil::quote_windows_arg`, `winutil::argv_to_command_line`,
//!   `cfg(windows)`).
//! - Cross-platform PTY/process driver adapter (`pty::ProcessDriver`,
//!   `pty::SpawnedProcess`, `pty::TerminalSize`, `pty::spawn_from_driver`).
//! - Restricted-token builders for AppContainer / capability sandboxing
//!   (`token::world_sid`, `token::convert_string_sid_to_sid`,
//!   `token::get_current_token_for_restriction`,
//!   `token::get_logon_sid_bytes`, `token::create_readonly_token_with_cap`,
//!   `token::create_readonly_token_with_cap_from`,
//!   `token::create_readonly_token_with_caps_from`,
//!   `token::create_workspace_write_token_with_caps_from`, `cfg(windows)`).
//! - Private-desktop launcher (`desktop::LaunchDesktop::prepare`,
//!   `desktop::LaunchDesktop::startup_info_desktop`, `cfg(windows)`).
//! - On non-Windows targets, [`unsupported`] returns a typed error indicating
//!   that the Windows sandbox runtime is unavailable.
//!
//! The actual Win32 sandbox logic (AppContainer, JobObject, network ACLs)
//! lands in subsequent PRs under tracker issue #263 / #250.

pub mod absolute_path;
pub mod cap;
#[cfg(windows)]
pub mod desktop;
#[cfg(windows)]
pub mod dpapi;
pub mod env;
pub mod logging;
pub mod path_normalization;
#[cfg(windows)]
pub mod proc_thread_attr;
pub mod pty;
pub mod sandbox_utils;
pub mod setup_error;
pub mod string_util;
#[cfg(windows)]
pub mod token;
pub mod types;
#[cfg(windows)]
pub mod winutil;

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
