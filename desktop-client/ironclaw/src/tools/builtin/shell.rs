//! Shell execution tool — re-export shim.
//!
//! F4.6.7 Phase 2+3 (ADR-156 §6.3): the body lives in
//! [`dasclaw_shell_tools`]. This module exists only to preserve the
//! `crate::tools::builtin::shell::*` path used by `builtin/mod.rs`.
//!
//! Migration history:
//! - Phase 1: sandbox-independent helpers (risk classification, injection
//!   detection, env scrub list, output truncation) moved out.
//! - Phase 2+3: full [`ShellTool`] struct + the sandbox executor shim
//!   (`OsExecutor` → [`dasclaw_shell_tools::SandboxedShellExecutor`])
//!   moved out. Desktop now constructs and threads the crate types
//!   directly.

pub use dasclaw_shell_tools::{ShellTool, classify_command_risk};
