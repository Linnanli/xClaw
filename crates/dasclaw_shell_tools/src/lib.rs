//! F4.6.7 Phase 2+3 — full ShellTool + sandboxed shell executor extracted
//! from `desktop-client/ironclaw` per ADR-156 §6.3.
//!
//! Phase 1 already moved the pure helpers (pattern tables, risk
//! classification, injection detection, env scrubbing list, output
//! truncation, intent labelling, parameter extraction). Phase 2+3 finishes
//! the job by sinking the [`ShellTool`] body itself and replacing the
//! desktop `OsExecutor` shim with [`SandboxedShellExecutor`], which talks
//! to [`dasclaw_exec::SandboxedExecutor`] directly using the 4-variant
//! [`dasclaw_workspace_cap::policy::SandboxPolicy`].
//!
//! Verbatim port (ironclaw → crate): bodies are byte-for-byte copies of
//! the desktop sources with mechanical rewrites only — types swapped from
//! `crate::sandbox::{OsExecutor, SandboxPolicy}` to the new in-crate
//! [`SandboxedShellExecutor`] + 4-variant `CapPolicy`, error types
//! changed to the in-crate [`ShellExecError`], and the
//! `crate::util::floor_char_boundary` polyfill replaced with the
//! standard-library `str::floor_char_boundary` (stable since 1.79).

mod helpers;
mod sandboxed_executor;
mod shell_tool;

pub use helpers::{
    BLOCKED_COMMANDS, DANGEROUS_PATTERNS, MAX_OUTPUT_SIZE, SAFE_ENV_VARS,
    analyze_command_for_result, classify_command_risk, detect_command_injection,
    extract_command_param, truncate_for_error, truncate_output,
};
pub use sandboxed_executor::{ExecOutput, SandboxedShellExecutor, ShellExecError};
pub use shell_tool::ShellTool;
