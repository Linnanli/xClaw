//! OS-level execution sandbox for secure command execution.
//!
//! This module provides sandboxing primitives for the agent's tool layer:
//!
//! - **OS-level isolation**: codex-style platform sandboxes
//!   (macOS Seatbelt / Linux Landlock+seccomp / Windows Restricted Token),
//!   delegated to `dasclaw_exec::SandboxedExecutor` and
//!   `dasclaw_sandbox::Sandbox`. F4.6.7 Phase 2+3: the previous
//!   `os_executor::OsExecutor` shim was sunk into
//!   [`dasclaw_shell_tools::SandboxedShellExecutor`]; this module now
//!   only owns config parsing and the network proxy.
//! - **Docker daemon detection** ([`detect`]): read-only probe used by the
//!   setup wizard, boot screen, and `ironclaw doctor` to surface Docker
//!   availability for the **separate** background-job container layer
//!   (`orchestrator/{job_manager,reaper}.rs`). It is no longer involved in
//!   tool execution.
//!
//! # Sandbox Policies
//!
//! | Policy | Filesystem | Network | Use Case |
//! |--------|------------|---------|----------|
//! | `ReadOnly` | Read workspace | Blocked | Explore code, fetch docs |
//! | `WorkspaceWrite` | Read/write workspace | Blocked | Build software, run tests |
//! | `FullAccess` | Full host | Full | Direct execution (no sandbox) |
//!
//! Network access for `ReadOnly` / `WorkspaceWrite` is blocked at the OS
//! sandbox layer (Seatbelt / Landlock+seccomp / Restricted Token). A future
//! audited HTTP proxy will be ported from `codex-network-proxy` into the
//! `dasclaw_net_proxy` crate (W7).

pub mod config;
pub mod detect;
pub mod docker_conn;
pub mod error;
/// W3.2b-4: bridge to [`dasclaw_net_proxy`] for sandbox egress enforcement.
pub mod net_proxy;

pub use config::{
    ExecutionMode, ResourceLimits, SandboxConfig, SandboxPolicy, legacy_policy_to_cap,
};
pub use detect::{DockerDetection, DockerStatus, Platform, check_docker};
pub use docker_conn::connect_docker;
pub use error::{Result, SandboxError};

/// Default allowlist getter (re-export for convenience).
pub fn default_allowlist() -> Vec<String> {
    config::default_allowlist()
}

/// Default credential mappings getter (re-export for convenience).
pub fn default_credential_mappings() -> Vec<dasclaw_runtime::secrets::CredentialMapping> {
    config::default_credential_mappings()
}
