//! OS-level execution sandbox for secure command execution.
//!
//! This module provides sandboxing primitives for the agent's tool layer:
//!
//! - **OS-level isolation** ([`os_executor`]): codex-style platform sandboxes
//!   (macOS Seatbelt / Linux Landlock+seccomp / Windows Restricted Token),
//!   delegated to `dasclaw_exec::SandboxedExecutor` and
//!   `dasclaw_sandbox::Sandbox`. Replaces the previous Docker-based execution
//!   path (`SandboxManager`, removed in W3.1c).
//! - **Network proxy** ([`proxy`]): allowlist enforcement with credential
//!   injection by domain. Used by tools that need outbound HTTP from agent
//!   code without exposing API keys to the executed command.
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
//! | `ReadOnly` | Read workspace | Proxied | Explore code, fetch docs |
//! | `WorkspaceWrite` | Read/write workspace | Proxied | Build software, run tests |
//! | `FullAccess` | Full host | Full | Direct execution (no sandbox) |

pub mod config;
pub mod detect;
pub mod docker_conn;
pub mod error;
/// W3.1a: codex-style OS-level executor (replaces Docker `SandboxManager` for
/// tool execution). See [`os_executor::OsExecutor`].
pub mod os_executor;
pub mod proxy;

pub use config::{ResourceLimits, SandboxConfig, SandboxPolicy};
pub use detect::{DockerDetection, DockerStatus, Platform, check_docker};
pub use docker_conn::connect_docker;
pub use error::{Result, SandboxError};
pub use os_executor::{ExecOutput, OsExecutor};
pub use proxy::{
    CredentialResolver, DefaultPolicyDecider, DomainAllowlist, EnvCredentialResolver, HttpProxy,
    NetworkDecision, NetworkPolicyDecider, NetworkProxyBuilder, NetworkRequest,
};

/// Default allowlist getter (re-export for convenience).
pub fn default_allowlist() -> Vec<String> {
    config::default_allowlist()
}

/// Default credential mappings getter (re-export for convenience).
pub fn default_credential_mappings() -> Vec<crate::secrets::CredentialMapping> {
    config::default_credential_mappings()
}
