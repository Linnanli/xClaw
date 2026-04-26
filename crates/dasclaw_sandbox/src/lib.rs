//! `dasclaw_sandbox` — Cross-platform sandbox public abstractions (W2.1).
//!
//! W2.1 scope: port the **public abstraction layer** from
//! `codex-cli-main/codex-rs/sandboxing/src/{lib,manager,policy_transforms}.rs`
//! and expose a neutral `Sandbox` trait that desktop-client and
//! `dasclaw_core` can depend on. Platform backends are stubbed in W2.1 and
//! filled in W2.2 (macOS seatbelt) → W2.3 (linux landlock) → W2.4 (windows
//! restricted token).
//!
//! Design notes:
//! - `SandboxType`, `SandboxablePreference`, `get_platform_sandbox()` mirror
//!   codex `manager.rs` API surface, neutralized (no `codex_protocol` dep).
//! - `SandboxPolicy` is a **dasclaw-owned** struct, not re-exported from
//!   codex-protocol. Mapping adapter lives in `dasclaw_bridge_lite` (W3+).
//! - Three platform backends gated by `cfg(target_os = ...)` like codex.
//! - Real impl deferred (W2.2+); current stubs return
//!   `SandboxError::NotImplemented` so callers compile but fail loud at runtime.

#![allow(dead_code)]

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::SeatbeltSandbox;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxSeccompSandbox;

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("sandbox backend not implemented yet (W2.{stage}): {detail}")]
    NotImplemented { stage: u8, detail: String },

    #[error("policy transform failed: {0}")]
    PolicyTransform(String),

    #[error("platform sandbox unavailable on this OS")]
    PlatformUnavailable,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// The actual sandbox backend that will run a command.
///
/// `#[non_exhaustive]` so adding new variants (e.g. FreeBSD Capsicum) in
/// W3+ doesn't silently break out-of-crate `match` sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SandboxType {
    None,
    MacosSeatbelt,
    LinuxSeccomp,
    WindowsRestrictedToken,
}

impl SandboxType {
    pub fn as_metric_tag(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MacosSeatbelt => "seatbelt",
            Self::LinuxSeccomp => "seccomp",
            Self::WindowsRestrictedToken => "windows_sandbox",
        }
    }
}

/// Whether the caller insists on running under a sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxablePreference {
    Auto,
    Require,
    Forbid,
}

/// Resolve the platform-default sandbox backend (mirrors codex
/// `get_platform_sandbox()`).
pub fn get_platform_sandbox(windows_sandbox_enabled: bool) -> Option<SandboxType> {
    if cfg!(target_os = "macos") {
        Some(SandboxType::MacosSeatbelt)
    } else if cfg!(target_os = "linux") {
        Some(SandboxType::LinuxSeccomp)
    } else if cfg!(target_os = "windows") {
        if windows_sandbox_enabled {
            Some(SandboxType::WindowsRestrictedToken)
        } else {
            None
        }
    } else {
        None
    }
}

/// Minimal neutral sandbox policy. Adapter to/from codex's richer enum lives
/// in `dasclaw_bridge_lite` (W3+).
#[derive(Clone, Debug, Default)]
pub struct SandboxPolicy {
    pub readable_roots: Vec<PathBuf>,
    pub writable_roots: Vec<PathBuf>,
    pub allow_network: bool,
    pub allow_spawn: bool,
    /// Loopback ports that the sandbox should allow outbound connect to,
    /// detected from `HTTP_PROXY` / `HTTPS_PROXY` env vars.
    /// Populated by `proxy::detect_loopback_ports()`.
    pub proxy_loopback_ports: Vec<u16>,
}

impl SandboxPolicy {
    pub fn read_only_defaults() -> Self {
        Self::default()
    }

    pub fn with_writable<P: Into<PathBuf>>(mut self, p: P) -> Self {
        self.writable_roots.push(p.into());
        self
    }

    pub fn with_proxy_ports(mut self, ports: Vec<u16>) -> Self {
        self.proxy_loopback_ports = ports;
        self
    }
}

pub mod proxy;

/// What the caller wants to execute under a sandbox.
#[derive(Debug)]
pub struct SandboxExecRequest {
    pub command: Command,
    pub policy: SandboxPolicy,
    pub preference: SandboxablePreference,
    pub windows_sandbox_enabled: bool,
}

/// Primary entry trait. Replaces W1 `SkeletonError`-returning placeholder.
pub trait Sandbox {
    fn kind(&self) -> SandboxType;
    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError>;
}

/// Resolve `pref` + platform → concrete backend.
pub fn select_backend(
    pref: SandboxablePreference,
    windows_sandbox_enabled: bool,
) -> Result<Box<dyn Sandbox>, SandboxError> {
    let resolved = match pref {
        SandboxablePreference::Forbid => SandboxType::None,
        SandboxablePreference::Auto | SandboxablePreference::Require => {
            match get_platform_sandbox(windows_sandbox_enabled) {
                Some(t) => t,
                None => {
                    if matches!(pref, SandboxablePreference::Require) {
                        return Err(SandboxError::PlatformUnavailable);
                    }
                    SandboxType::None
                }
            }
        }
    };

    Ok(build_backend(resolved))
}

/// Map a resolved `SandboxType` to a concrete `Box<dyn Sandbox>`.
///
/// Real backends are gated behind their respective `cfg(target_os = ...)`.
/// On a target whose backend is not yet implemented (W2.3 Linux, W2.4
/// Windows), `NoopSandbox` is returned; its `execute()` will surface
/// `SandboxError::NotImplemented` at run-time.
fn build_backend(t: SandboxType) -> Box<dyn Sandbox> {
    #[cfg(target_os = "macos")]
    if matches!(t, SandboxType::MacosSeatbelt) {
        return Box::new(SeatbeltSandbox::new());
    }
    #[cfg(target_os = "linux")]
    if matches!(t, SandboxType::LinuxSeccomp) {
        return Box::new(LinuxSeccompSandbox::new());
    }
    Box::new(NoopSandbox { kind: t })
}

/// W2.1 stub. Reports its `kind` honestly but `execute()` fails fast unless
/// `kind == None`.
pub struct NoopSandbox {
    kind: SandboxType,
}

impl Sandbox for NoopSandbox {
    fn kind(&self) -> SandboxType {
        self.kind
    }

    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError> {
        match self.kind {
            SandboxType::None => {
                let mut command = req.command;
                Ok(command.output()?)
            }
            SandboxType::MacosSeatbelt => Err(SandboxError::NotImplemented {
                stage: 2,
                detail: "seatbelt backend lands in W2.2".into(),
            }),
            SandboxType::LinuxSeccomp => Err(SandboxError::NotImplemented {
                stage: 3,
                detail: "linux landlock+bwrap backend lands in W2.3".into(),
            }),
            SandboxType::WindowsRestrictedToken => Err(SandboxError::NotImplemented {
                stage: 4,
                detail: "windows restricted token backend lands in W2.4".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_tag_is_stable() {
        assert_eq!(SandboxType::None.as_metric_tag(), "none");
        assert_eq!(SandboxType::MacosSeatbelt.as_metric_tag(), "seatbelt");
        assert_eq!(SandboxType::LinuxSeccomp.as_metric_tag(), "seccomp");
        assert_eq!(
            SandboxType::WindowsRestrictedToken.as_metric_tag(),
            "windows_sandbox"
        );
    }

    #[test]
    fn platform_sandbox_resolution() {
        let resolved = get_platform_sandbox(true);
        if cfg!(any(target_os = "macos", target_os = "linux")) {
            assert!(resolved.is_some());
        }
    }

    #[test]
    fn forbid_yields_none_kind() {
        let sb = select_backend(SandboxablePreference::Forbid, false).expect("Forbid path");
        assert_eq!(sb.kind(), SandboxType::None);
    }

    #[test]
    fn require_on_unsupported_returns_unavailable() {
        if cfg!(target_os = "windows") {
            let r = select_backend(SandboxablePreference::Require, false);
            assert!(matches!(r, Err(SandboxError::PlatformUnavailable)));
        }
    }

    #[test]
    fn noop_with_none_kind_runs_command() {
        let sb = NoopSandbox {
            kind: SandboxType::None,
        };
        let cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", "exit 0"]);
            c
        } else {
            Command::new("true")
        };
        let req = SandboxExecRequest {
            command: cmd,
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Forbid,
            windows_sandbox_enabled: false,
        };
        let out = sb.execute(req).expect("kind=None should run");
        assert!(out.status.success());
    }

    #[test]
    fn noop_with_seatbelt_kind_returns_not_implemented() {
        let sb = NoopSandbox {
            kind: SandboxType::MacosSeatbelt,
        };
        let req = SandboxExecRequest {
            command: Command::new("true"),
            policy: SandboxPolicy::read_only_defaults(),
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
        };
        let err = sb.execute(req).unwrap_err();
        assert!(matches!(err, SandboxError::NotImplemented { stage: 2, .. }));
    }

    #[test]
    fn policy_builder_adds_writable_root() {
        let p = SandboxPolicy::read_only_defaults().with_writable("/tmp/work");
        assert_eq!(p.writable_roots.len(), 1);
        assert_eq!(p.writable_roots[0], PathBuf::from("/tmp/work"));
        assert!(!p.allow_network);
        assert!(!p.allow_spawn);
    }
}
