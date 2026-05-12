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
//! - `SandboxBackendConfig` is a **dasclaw-owned** struct describing the
//!   *backend-level* knobs the sandbox kernel needs (writable roots, proxy
//!   ports, network/spawn flags). It is intentionally *flatter* than the
//!   codex `SandboxPolicy` enum — the higher-level policy enum lives in
//!   `ironclaw_workspace_cap::policy::SandboxPolicy` and is converted to
//!   `SandboxBackendConfig` by `dasclaw_exec` at execution time.
//!   `SandboxPolicy` remains as a **deprecated alias** for transition.
//! - Three platform backends gated by `cfg(target_os = ...)` like codex.
//! - Real impl deferred (W2.2+); current stubs return
//!   `SandboxError::NotImplemented` so callers compile but fail loud at runtime.

#![allow(dead_code)]

/// ADR-131 / Slice B1 — IPC protocol for `dasclaw-sandbox-resource-launcher`.
///
/// 跨平台公开（serde 数据结构），便于本地测试与 macOS/Linux 构建检查通过。
pub mod launcher_ipc;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::SeatbeltSandbox;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxSeccompSandbox;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsRestrictedTokenSandbox;

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

    /// Windows sandbox infrastructure (`dasclaw-sandbox-setup.exe`) has not
    /// been initialized yet. Distinct from `NotImplemented` so callers can
    /// surface a setup-prompt UX instead of treating the platform as
    /// unsupported. See ADR-121 D3-3 / Phase 1.2 (issue #320).
    #[error("windows sandbox setup pending: {detail}")]
    WindowsSetupPending { detail: String },

    /// Windows resource-limits launcher (`dasclaw-sandbox-resource-launcher.exe`)
    /// failed at spawn / IPC / Job Object FFI / sub-process invocation.
    /// **Distinct** from [`Self::WindowsSetupPending`]: setup is already done,
    /// but this particular invocation's wrapper failed. Callers should not
    /// prompt the user to re-run setup. See ADR-131 §7 Q2.
    #[error("windows sandbox resource launcher failed: {detail}")]
    WindowsLauncherFailed { detail: String },

    /// Enterprise mode is on, but no OS-level sandbox is wired into the
    /// spawn path on this platform. Returned by [`check_enterprise_gate`]
    /// and surfaced by backends (currently Windows) when the platform's
    /// kernel sandbox infrastructure is not yet available.
    ///
    /// **Fail-closed**: callers must not silently downgrade to direct
    /// `exec`. See [ADR-141](../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md)
    /// §3 PR-W1 / §6 OQ-2.
    #[error("windows enterprise mode requires an OS sandbox but none is wired: {detail}")]
    WindowsSandboxNotAvailable { detail: String },

    /// Enterprise mode is on and the spawn carries workspace-write carve-outs
    /// (e.g. `.git`, `.codex`, `.dasclaw`), but this platform's kernel-layer
    /// `read_only_subpaths` enforcement has not been ported yet. Coverage:
    /// macOS ✅ Wave-C1a; Windows DACL DENY data path ✅ wired by PR #426
    /// (Wave-C1b) but `check_enterprise_gate` Step 3 arm not yet realigned —
    /// tracked in [#434](https://github.com/Linnanli/xClaw/issues/434); Linux
    /// 🔴 waits on Wave-C1c.
    ///
    /// **Fail-closed by default**. Callers may downgrade to user-space-only
    /// enforcement by setting
    /// [`SandboxBackendConfig::enterprise_allow_userspace_carveouts`] to
    /// `true` and emitting a structured per-spawn audit event. See
    /// [ADR-141](../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md)
    /// §3 PR-W2 / §6 OQ-1 / OQ-4.
    #[error(
        "enterprise mode requires kernel-layer read_only_subpaths enforcement, \
         which is not yet ported on this platform: {detail}"
    )]
    ReadOnlySubpathsKernelEnforcementMissing { detail: String },

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

/// Per-process resource limits enforced via `setrlimit(2)` on Unix and
/// outer Job Object via the `dasclaw-sandbox-resource-launcher` binary on
/// Windows (see [ADR-131](../../docs/plans/architecture-refactor/adr-131-windows-job-object-resource-limits-wrapper.md)).
///
/// Each field is `Option<u64>`; `None` means "do not enforce" (inherit
/// parent limit). [`ResourceLimits::default`] returns sane defaults that
/// handle 99% of legitimate workloads while blocking runaway scripts and
/// fork bombs before the OS OOM killer mis-targets unrelated processes.
///
/// Callers wanting **no enforcement** must explicitly call
/// [`ResourceLimits::unlimited`].
///
/// See [45 — Resource Limits ADR](../../docs/plans/architecture-refactor/45-resource-limits-adr.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum address-space size in bytes. Enforced via `RLIMIT_AS` on
    /// Unix; `JOB_OBJECT_LIMIT_PROCESS_MEMORY` on Windows (outer Job Object
    /// set by the launcher binary; see ADR-131).
    pub max_memory_bytes: Option<u64>,

    /// Maximum CPU seconds (soft = `SIGXCPU`, hard = `SIGKILL`). Enforced
    /// via `RLIMIT_CPU` on Unix; silently ignored on Windows.
    pub max_cpu_secs: Option<u64>,

    /// Max simultaneously open file descriptors. Enforced via
    /// `RLIMIT_NOFILE` on Unix; silently ignored on Windows.
    pub max_open_files: Option<u64>,

    /// Max child processes the sandbox may spawn. Enforced via
    /// `RLIMIT_NPROC` on Unix; `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` on Windows.
    pub max_processes: Option<u64>,
}

impl Default for ResourceLimits {
    /// Sane defaults: 4 GiB memory / 600 s CPU / 1024 FDs / 1024 processes.
    ///
    /// Rationale per Round 20 user decision: macOS OOM killer sometimes
    /// mis-targets Finder/IDE under memory pressure; defaults catch
    /// runaway scripts before that triggers. Users override via config.
    fn default() -> Self {
        Self {
            max_memory_bytes: Some(4 * 1024 * 1024 * 1024), // 4 GiB
            max_cpu_secs: Some(600),                        // 10 min
            max_open_files: Some(1024),
            max_processes: Some(1024),
        }
    }
}

impl ResourceLimits {
    /// Construct a [`ResourceLimits`] with **no** enforcement. Callers
    /// should prefer [`Default::default`] unless they have a specific
    /// reason to disable all limits (e.g. trusted batch jobs).
    pub fn unlimited() -> Self {
        Self {
            max_memory_bytes: None,
            max_cpu_secs: None,
            max_open_files: None,
            max_processes: None,
        }
    }
}

/// Backend-level sandbox configuration: the concrete knobs the sandbox
/// **kernel** (seatbelt / seccomp / windows) needs to enforce a policy.
///
/// This struct is intentionally flatter than codex's `SandboxPolicy` enum.
/// The higher-level policy enum lives in
/// `ironclaw_workspace_cap::policy::SandboxPolicy` and is converted to this
/// struct by `dasclaw_exec` (or any caller) at execution time.
#[derive(Clone, Debug, Default)]
pub struct SandboxBackendConfig {
    pub readable_roots: Vec<PathBuf>,
    pub writable_roots: Vec<PathBuf>,
    /// Subpaths that must remain **read-only** even when contained inside a
    /// [`Self::writable_roots`] entry — the so-called *洞中洞* / "holes within
    /// holes" carve-outs (`.git/`, `.dasclaw/`, `.codex/`, etc.).
    ///
    /// Populated by `dasclaw_exec::policy_to_backend_config_with_env` from
    /// [`ironclaw_workspace_cap::policy::WritableRoot::read_only_subpaths`]
    /// for every writable root.
    ///
    /// **Per-backend semantics** (ADR-141 §3 PR-W3, OQ-W3-1/W3-4 sign-off
    /// 2026-05-11):
    /// - **Windows** (`crates/dasclaw_sandbox/src/windows/`) — kernel-enforced
    ///   via Win32 DACL DENY ACEs. The list is forwarded into the launcher
    ///   IPC ([`crate::launcher_ipc::LauncherRequest::additional_deny_write_paths`])
    ///   and consumed by upstream
    ///   `dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths`.
    /// - **macOS** (`crates/dasclaw_sandbox/src/macos/`) — **ignored** at this
    ///   layer; sbpl-level enforcement is wired through ADR-135 §3 PR-C1
    ///   (`dasclaw_sandboxing::seatbelt`) using the higher-level
    ///   `WritableRoot { root, read_only_subpaths }` shape directly. Keeping
    ///   the field on `SandboxBackendConfig` lets [`check_enterprise_gate`]
    ///   reason about the *presence* of carve-outs uniformly across hosts.
    /// - **Linux** — ignored until Wave-C1c lands kernel-layer Landlock
    ///   enforcement (ADR-135 PR-C3); behaviour mirrors macOS for now.
    pub read_only_subpaths: Vec<PathBuf>,
    pub allow_network: bool,
    pub allow_spawn: bool,
    /// Loopback ports that the sandbox should allow outbound connect to,
    /// detected from `HTTP_PROXY` / `HTTPS_PROXY` env vars.
    /// Populated by `proxy::detect_loopback_ports()`.
    pub proxy_loopback_ports: Vec<u16>,
    /// Resource limits applied via `setrlimit` in the child process's
    /// pre_exec hook. Defaults to [`ResourceLimits::default`] (4 GiB / 600 s
    /// / 1024 FDs / 1024 procs). Pass [`ResourceLimits::unlimited`] to opt
    /// out completely. See W3.3 ADR.
    pub resource_limits: ResourceLimits,

    /// Whether the calling session is governed by **enterprise mode**
    /// (i.e. an organization-managed fail-closed policy applies).
    ///
    /// When `false` (default), backends behave as before; the gate in
    /// [`check_enterprise_gate`] short-circuits to
    /// [`EnterpriseGateOutcome::Allow`].
    ///
    /// When `true`, backends must run the gate at the top of `execute` and
    /// reject spawns whose required kernel-layer enforcement is not yet
    /// ported on the host platform. See
    /// [ADR-141](../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md)
    /// §3 PR-W1 / §6 OQ-1.
    pub enterprise_mode: bool,

    /// **Soft-mode opt-in** for enterprise mode. When `enterprise_mode = true`
    /// **and** the host platform lacks kernel-layer `read_only_subpaths`
    /// enforcement, this flag lets the caller proceed with user-space-only
    /// carve-outs (via [`ironclaw_workspace_cap::WorkspaceCap`] cap-std
    /// interception + `is_path_writable`) **provided that** a structured
    /// per-spawn audit event is emitted by the caller.
    ///
    /// Defaults to `false` (fail-closed) per ADR-141 §6 OQ-1.
    pub enterprise_allow_userspace_carveouts: bool,
}

impl SandboxBackendConfig {
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

    /// Override the resource limits applied to spawned processes.
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    /// Enable enterprise mode (fail-closed gate). See [ADR-141][adr-141].
    ///
    /// [adr-141]: ../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md
    pub fn with_enterprise_mode(mut self, enabled: bool) -> Self {
        self.enterprise_mode = enabled;
        self
    }

    /// Opt into user-space-only carve-out enforcement when the host
    /// platform's kernel does not yet enforce `read_only_subpaths`. Must be
    /// paired with a structured per-spawn audit event by the caller. See
    /// [ADR-141][adr-141] §6 OQ-1 / OQ-4.
    ///
    /// [adr-141]: ../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md
    pub fn with_enterprise_allow_userspace_carveouts(mut self, allow: bool) -> Self {
        self.enterprise_allow_userspace_carveouts = allow;
        self
    }
}

/// Reason a soft-mode allow was granted by [`check_enterprise_gate`].
///
/// Carried in [`EnterpriseGateOutcome::AllowWithUserspaceSoftMode`] so callers
/// can emit a structured per-spawn audit event (ADR-141 §6 OQ-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SoftModeReason {
    /// Windows: retained for `check_enterprise_gate` symmetry with Linux,
    /// but the underlying premise (kernel-layer `read_only_subpaths` not
    /// available) is now stale — ADR-141 §3 PR-W3 / Wave-C1b landed via
    /// PR #426 wired `read_only_subpaths` to Win32 DACL DENY entries.
    /// The gate Step 3 Windows arm has **not** yet been updated to
    /// acknowledge this; tracked in
    /// [#434](https://github.com/Linnanli/xClaw/issues/434) for a
    /// dedicated decision PR (failure-mode semantic change requires
    /// nally sign-off; out of scope for purely doc-correcting PRs).
    WindowsNoKernelReadOnlySubpaths,
    /// Linux lacks kernel-layer `read_only_subpaths` enforcement until
    /// Wave-C1c (`dasclaw-linux-sandbox` setuid binary) lands.
    LinuxNoKernelReadOnlySubpaths,
}

impl SoftModeReason {
    /// Stable, dot-namespaced tag suitable for log fields / metric labels.
    pub fn as_audit_tag(self) -> &'static str {
        match self {
            Self::WindowsNoKernelReadOnlySubpaths => "windows.no_kernel_read_only_subpaths",
            Self::LinuxNoKernelReadOnlySubpaths => "linux.no_kernel_read_only_subpaths",
        }
    }
}

/// Outcome of the ADR-141 enterprise fail-closed gate.
///
/// Encodes the four-way decision matrix from ADR-141 §3 PR-W1+W2 as a closed
/// enum so callers `match` exhaustively — there is no silent fall-through.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnterpriseGateOutcome {
    /// Either enterprise mode is off, or every required kernel-layer
    /// enforcement is already in place. Caller may proceed with the spawn.
    Allow,
    /// Enterprise mode is on, kernel-layer `read_only_subpaths` is not yet
    /// ported on this platform, **and** the caller opted into soft mode via
    /// [`SandboxBackendConfig::enterprise_allow_userspace_carveouts`].
    ///
    /// Caller **must** emit a structured per-spawn audit event before
    /// continuing (ADR-141 §6 OQ-3 / OQ-4). The `reason` is the audit tag.
    AllowWithUserspaceSoftMode { reason: SoftModeReason },
    /// Enterprise mode is on and the OS sandbox infrastructure is missing
    /// (Windows setup not run, or no sandbox backend at all). Caller must
    /// refuse to spawn with
    /// [`SandboxError::WindowsSandboxNotAvailable`].
    DenyNoKernelSandbox,
    /// Enterprise mode is on, kernel-layer carve-outs are not ported on this
    /// platform, and the soft flag is **not** set. Caller must refuse with
    /// [`SandboxError::ReadOnlySubpathsKernelEnforcementMissing`].
    DenyNoReadOnlySubpathsKernelEnforcement,
}

/// Cross-platform pure decision function for the ADR-141 enterprise
/// fail-closed gate.
///
/// **No side effects** — designed so the full decision matrix can be unit
/// tested on any host (macOS / Linux CI runners do not need a Windows VM).
///
/// Inputs:
/// - `config`: the resolved [`SandboxBackendConfig`]; only the two
///   `enterprise_*` fields and `writable_roots` affect the decision.
/// - `sandbox_type`: the platform backend that will actually run the spawn.
/// - `kernel_setup_complete`: the platform-specific "sandbox infra ready"
///   signal. On Windows this is `sandbox_setup_is_complete(dasclaw_home)`;
///   on macOS/Linux it should be passed as `true` (the kernel is always
///   available — there is no per-machine setup step).
///
/// See [ADR-141](../../docs/plans/architecture-refactor/adr-141-windows-enterprise-sandbox-support.md)
/// §1.1 for the platform-by-platform support matrix that drives this logic.
pub fn check_enterprise_gate(
    config: &SandboxBackendConfig,
    sandbox_type: SandboxType,
    kernel_setup_complete: bool,
) -> EnterpriseGateOutcome {
    if !config.enterprise_mode {
        return EnterpriseGateOutcome::Allow;
    }

    // Step 1: a kernel sandbox must exist and be set up on this platform.
    if sandbox_type == SandboxType::None || !kernel_setup_complete {
        return EnterpriseGateOutcome::DenyNoKernelSandbox;
    }

    // Step 2: if the spawn carries no `read_only_subpaths` carve-outs,
    // there is nothing for the kernel layer to extra-deny — the base
    // sandbox kind alone is sufficient regardless of how many writable
    // roots are present (a workspace-write without any holes is the
    // common case for non-enterprise tasks).
    //
    // OQ-W3-3 (sign-off 2026-05-11) corrected the original PR-W1+W2
    // approximation `writable_roots.is_empty()`: a `WorkspaceWrite` policy
    // *always* has writable roots, so the old signal misclassified the
    // hole-free case as "needs kernel enforcement".
    if config.read_only_subpaths.is_empty() {
        return EnterpriseGateOutcome::Allow;
    }

    // Step 3: kernel-layer `read_only_subpaths` enforcement matrix
    // (ADR-141 §1.1). macOS is wired in Wave-C1a via
    // `dasclaw_sandboxing::seatbelt`; on Windows the DACL DENY data path
    // is wired by PR #426 (Wave-C1b) but this arm still routes through
    // soft-mode/deny — realignment tracked in xClaw#434. Linux waits on
    // Wave-C1c.
    match sandbox_type {
        SandboxType::MacosSeatbelt => EnterpriseGateOutcome::Allow,
        SandboxType::WindowsRestrictedToken => {
            decide_carveout_outcome(config, SoftModeReason::WindowsNoKernelReadOnlySubpaths)
        }
        SandboxType::LinuxSeccomp => {
            decide_carveout_outcome(config, SoftModeReason::LinuxNoKernelReadOnlySubpaths)
        }
        SandboxType::None => EnterpriseGateOutcome::DenyNoKernelSandbox,
    }
}

fn decide_carveout_outcome(
    config: &SandboxBackendConfig,
    reason: SoftModeReason,
) -> EnterpriseGateOutcome {
    if config.enterprise_allow_userspace_carveouts {
        EnterpriseGateOutcome::AllowWithUserspaceSoftMode { reason }
    } else {
        EnterpriseGateOutcome::DenyNoReadOnlySubpathsKernelEnforcement
    }
}

/// Deprecated alias kept for transition; will be removed once all call
/// sites use `SandboxBackendConfig` directly.
pub type SandboxPolicy = SandboxBackendConfig;

pub mod proxy;

#[cfg(unix)]
pub mod rlimit;

/// What the caller wants to execute under a sandbox.
#[derive(Debug)]
pub struct SandboxExecRequest {
    pub command: Command,
    pub policy: SandboxBackendConfig,
    pub preference: SandboxablePreference,
    pub windows_sandbox_enabled: bool,
    /// Optional reference to the managed `NetworkProxy` for the calling
    /// session. macOS Seatbelt forwards this to
    /// `dasclaw_sandboxing::seatbelt::create_seatbelt_command_args` so the
    /// resulting sbpl includes `(allow network-outbound (remote ip ...))`
    /// rules for the proxy's loopback HTTP/SOCKS ports (a.k.a. "hole
    /// punching" under `allow_network=false`). When `None`, no proxy
    /// hole-punch rules are emitted — matches Wave-C1 behaviour and is
    /// safe (fail-closed). See ADR-135 §3 PR-C2 / ADR-142.
    pub network: Option<dasclaw_net_proxy::NetworkProxy>,
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
    #[cfg(target_os = "windows")]
    if matches!(t, SandboxType::WindowsRestrictedToken) {
        return Box::new(WindowsRestrictedTokenSandbox::new());
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
            network: None,
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
            network: None,
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

    // ---- ADR-141 fail-closed gate ---------------------------------------
    //
    // These tests cover the full `check_enterprise_gate` decision matrix.
    // They are cross-platform pure-function tests (no `cfg(target_os = ...)`)
    // so the same matrix is verified on every CI host.

    fn gate_cfg(
        enterprise: bool,
        soft: bool,
        writable: &[&str],
        read_only_subpaths: &[&str],
    ) -> SandboxBackendConfig {
        SandboxBackendConfig {
            enterprise_mode: enterprise,
            enterprise_allow_userspace_carveouts: soft,
            writable_roots: writable.iter().map(PathBuf::from).collect(),
            read_only_subpaths: read_only_subpaths.iter().map(PathBuf::from).collect(),
            ..SandboxBackendConfig::default()
        }
    }

    #[test]
    fn req_enterprise_gate_off_always_allows() {
        for &sb in &[
            SandboxType::None,
            SandboxType::MacosSeatbelt,
            SandboxType::LinuxSeccomp,
            SandboxType::WindowsRestrictedToken,
        ] {
            let cfg = gate_cfg(false, false, &["/tmp/work"], &["/tmp/work/.git"]);
            assert_eq!(
                check_enterprise_gate(&cfg, sb, false),
                EnterpriseGateOutcome::Allow,
                "enterprise_mode=false must short-circuit on {sb:?}",
            );
        }
    }

    #[test]
    fn req_enterprise_gate_denies_when_no_kernel_sandbox() {
        let cfg = gate_cfg(true, false, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::None, true),
            EnterpriseGateOutcome::DenyNoKernelSandbox,
        );
    }

    #[test]
    fn req_enterprise_gate_denies_when_windows_setup_pending() {
        let cfg = gate_cfg(true, false, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::WindowsRestrictedToken, false),
            EnterpriseGateOutcome::DenyNoKernelSandbox,
            "Windows with !kernel_setup_complete must Deny",
        );
    }

    #[test]
    fn req_enterprise_gate_macos_allows_with_workspace_write() {
        let cfg = gate_cfg(true, false, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::MacosSeatbelt, true),
            EnterpriseGateOutcome::Allow,
            "Wave-C1a wired macOS kernel carve-outs",
        );
    }

    #[test]
    fn req_enterprise_gate_windows_denies_carveouts_without_soft_flag() {
        let cfg = gate_cfg(true, false, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::WindowsRestrictedToken, true),
            EnterpriseGateOutcome::DenyNoReadOnlySubpathsKernelEnforcement,
        );
    }

    #[test]
    fn req_enterprise_gate_windows_softmode_allows_with_reason() {
        let cfg = gate_cfg(true, true, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::WindowsRestrictedToken, true),
            EnterpriseGateOutcome::AllowWithUserspaceSoftMode {
                reason: SoftModeReason::WindowsNoKernelReadOnlySubpaths,
            },
        );
    }

    #[test]
    fn req_enterprise_gate_linux_denies_carveouts_without_soft_flag() {
        let cfg = gate_cfg(true, false, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::LinuxSeccomp, true),
            EnterpriseGateOutcome::DenyNoReadOnlySubpathsKernelEnforcement,
        );
    }

    #[test]
    fn req_enterprise_gate_linux_softmode_allows_with_reason() {
        let cfg = gate_cfg(true, true, &["/tmp/work"], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(&cfg, SandboxType::LinuxSeccomp, true),
            EnterpriseGateOutcome::AllowWithUserspaceSoftMode {
                reason: SoftModeReason::LinuxNoKernelReadOnlySubpaths,
            },
        );
    }

    #[test]
    fn req_enterprise_gate_no_read_only_subpaths_allows_everywhere() {
        // ADR-141 §3 PR-W3 / OQ-W3-3 (sign-off 2026-05-11): without any
        // `read_only_subpaths` carve-outs, the gate must not block on
        // missing kernel-layer hole enforcement — the base sandbox kind
        // already covers the request. This holds even when WorkspaceWrite
        // grants writable roots (the common non-enterprise hole-free case).
        for &sb in &[
            SandboxType::MacosSeatbelt,
            SandboxType::LinuxSeccomp,
            SandboxType::WindowsRestrictedToken,
        ] {
            let cfg = gate_cfg(true, false, &["/tmp/work"], &[]);
            assert_eq!(
                check_enterprise_gate(&cfg, sb, true),
                EnterpriseGateOutcome::Allow,
                "no read_only_subpaths ⇒ no carve-outs ⇒ Allow on {sb:?}",
            );
        }
    }

    #[test]
    fn req_enterprise_gate_signal_is_subpaths_not_writable_roots() {
        // OQ-W3-3 regression test: previously the gate short-circuited on
        // `writable_roots.is_empty()`, which misclassified the common
        // WorkspaceWrite hole-free case as "needs kernel enforcement".
        // After the OQ-W3-3 fix, the signal is `read_only_subpaths.is_empty()`.
        let cfg_holes_no_roots = gate_cfg(true, false, &[], &["/tmp/work/.git"]);
        assert_eq!(
            check_enterprise_gate(
                &cfg_holes_no_roots,
                SandboxType::WindowsRestrictedToken,
                true
            ),
            EnterpriseGateOutcome::DenyNoReadOnlySubpathsKernelEnforcement,
            "holes without writable_roots still demand kernel enforcement",
        );
    }

    #[test]
    fn req_softmode_reason_audit_tag_is_stable() {
        // Tag strings are part of the audit-log contract (ADR-141 §6 OQ-4).
        // Changing them is a downstream breaking change.
        assert_eq!(
            SoftModeReason::WindowsNoKernelReadOnlySubpaths.as_audit_tag(),
            "windows.no_kernel_read_only_subpaths",
        );
        assert_eq!(
            SoftModeReason::LinuxNoKernelReadOnlySubpaths.as_audit_tag(),
            "linux.no_kernel_read_only_subpaths",
        );
    }

    #[test]
    fn enterprise_builders_set_fields() {
        let cfg = SandboxBackendConfig::default()
            .with_enterprise_mode(true)
            .with_enterprise_allow_userspace_carveouts(true);
        assert!(cfg.enterprise_mode);
        assert!(cfg.enterprise_allow_userspace_carveouts);
    }
}
