//! OS-level execution sandbox for secure command execution.
//!
//! This module provides sandboxing primitives for the agent's tool layer:
//!
//! - **OS-level isolation** ([`os_executor`]): codex-style platform sandboxes
//!   (macOS Seatbelt / Linux Landlock+seccomp / Windows Restricted Token),
//!   delegated to `dasclaw_exec::SandboxedExecutor` and
//!   `dasclaw_sandbox::Sandbox`. Replaces the previous Docker-based execution
//!   path (`SandboxManager`, removed in W3.1c).
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
/// W3.1a: codex-style OS-level executor (replaces Docker `SandboxManager` for
/// tool execution). See [`os_executor::OsExecutor`].
pub mod os_executor;

pub use config::{ExecutionMode, ResourceLimits, SandboxConfig, SandboxPolicy};
pub use detect::{DockerDetection, DockerStatus, Platform, check_docker};
pub use docker_conn::connect_docker;
pub use error::{Result, SandboxError};
pub use os_executor::{ExecOutput, OsExecutor};

/// Default allowlist getter (re-export for convenience).
pub fn default_allowlist() -> Vec<String> {
    config::default_allowlist()
}

/// Default credential mappings getter (re-export for convenience).
pub fn default_credential_mappings() -> Vec<crate::secrets::CredentialMapping> {
    config::default_credential_mappings()
}
