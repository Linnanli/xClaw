//! Windows restricted-token sandbox backend (W2.4 / Phase 1.2).
//!
//! ## 范围
//!
//! 这是一层 **dasclaw-owned adapter**（不是 codex 端口），把 dasclaw 的
//! 跨平台抽象（[`crate::Sandbox`] / [`crate::SandboxExecRequest`] /
//! [`crate::SandboxBackendConfig`]）桥接到 `dasclaw_sandbox_windows` crate
//! 暴露的 [`run_windows_sandbox_capture`] 入口。
//!
//! `dasclaw_sandbox_windows` 自身是 codex `windows-sandbox-rs` 的 1:1 verbatim
//! port（commit `6e838a19fa`，详见 ADR-129 / ADR-130 与 Phase 1.1 完成报告
//! `docs/plans/architecture-refactor/49-sandbox-windows-phase-1.1.4j-completion.md`）。
//! 真正的 sandbox 实施（用户隔离 / Job Object / Restricted Token / Firewall /
//! Alternate Desktop / DPAPI / ConPTY）都在那个 crate 里。
//!
//! ## 转换契约
//!
//! [`SandboxBackendConfig`] → 上游 `windows_sandbox::SandboxPolicy`：
//!
//! | dasclaw `SandboxBackendConfig` | 上游 `SandboxPolicy` |
//! |--------------------------------|----------------------|
//! | `writable_roots` 空            | `ReadOnly { network_access }` |
//! | `writable_roots` 非空          | `WorkspaceWrite { writable_roots, network_access, .. }` |
//!
//! `proxy_loopback_ports` 暂不传给 Windows 后端（上游 `windows-sandbox-rs`
//! 通过 per-user firewall 表达网络限制，proxy hole-punch 由调用方在
//! `allow_network=true` 时显式开放）。后续若要支持 proxy-only 模式，参考
//! macOS Seatbelt 路径在 `crate::macos::SeatbeltSandbox::compose_policy` 中
//! 的处理。
//!
//! ## codex_home / dasclaw_home
//!
//! 上游 API 收一个 `codex_home: &Path`，是 setup 阶段写入 `.sandbox/` 的根目录
//! （DPAPI key、user secrets、cap SIDs marker 等）。dasclaw 复用这个变量名是
//! 因为 `dasclaw_sandbox_windows` 是 verbatim port。adapter 在运行时按以下
//! 顺序解析：
//!
//! 1. 显式 `DASCLAW_HOME` 环境变量
//! 2. 兼容上游 `CODEX_HOME` 环境变量（方便 dev 复用既有 setup 状态）
//! 3. fallback：[`dirs::config_dir`] `/dasclaw`
//!
//! ## 失败语义
//!
//! - 未 setup（`sandbox_setup_is_complete()` 返回 `false`）→
//!   [`crate::SandboxError::WindowsSetupPending`]，**不**走 `NotImplemented`，
//!   方便调用方区分 "OS 不支持" 与 "需要先跑 dasclaw-sandbox-setup.exe"。
//! - 序列化失败 → [`crate::SandboxError::PolicyTransform`]。
//! - 上游运行错误 → [`crate::SandboxError::Io`]（包成 io::Error，保留
//!   原始 message）。

#![cfg(target_os = "windows")]

pub mod job_object;
mod launcher_client;

use std::collections::HashMap;
use std::path::PathBuf;

use dasclaw_sandbox_windows::absolute_path::AbsolutePathBuf;
use dasclaw_sandbox_windows::sandbox_setup_is_complete;
use dasclaw_sandbox_windows::types::{NetworkAccess, SandboxPolicy as UpstreamPolicy};

use crate::launcher_ipc::{LauncherRequest, OuterJobLimitsWire, PROTOCOL_VERSION};
use crate::{
    check_enterprise_gate, EnterpriseGateOutcome, ResourceLimits, Sandbox, SandboxBackendConfig,
    SandboxError, SandboxExecRequest, SandboxType,
};

/// Windows restricted-token sandbox backend.
///
/// 调用 [`dasclaw_sandbox_windows::run_windows_sandbox_capture`] 在专用本地用户
/// 下用 Restricted Token + Job Object + Alternate Desktop + per-user Firewall
/// 隔离运行命令。详见 crate 文档与 ADR-121 D3-3。
pub struct WindowsRestrictedTokenSandbox;

impl WindowsRestrictedTokenSandbox {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsRestrictedTokenSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox for WindowsRestrictedTokenSandbox {
    fn kind(&self) -> SandboxType {
        SandboxType::WindowsRestrictedToken
    }

    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError> {
        let dasclaw_home = resolve_dasclaw_home()?;
        let setup_complete = sandbox_setup_is_complete(&dasclaw_home);

        // ADR-141 fail-closed gate (PR-W1). Runs first so that enterprise
        // spawns surface the correct error variant before any command
        // decomposition / IPC work. After xClaw#434 realigned Step 3 to
        // recognise PR #426 (Wave-C1b) DACL DENY enforcement, the gate's
        // Windows arm only produces `Allow` or `DenyNoKernelSandbox` —
        // the soft-mode / carve-out-deny variants are Linux-only until
        // Wave-C1c. The fallback arm guards against future regressions.
        match check_enterprise_gate(
            &req.policy,
            SandboxType::WindowsRestrictedToken,
            setup_complete,
        ) {
            EnterpriseGateOutcome::Allow => {}
            EnterpriseGateOutcome::DenyNoKernelSandbox => {
                return Err(SandboxError::WindowsSandboxNotAvailable {
                    detail: format!(
                        "Windows enterprise mode requires the OS sandbox to be set up at {} \
                         (run dasclaw-sandbox-setup.exe elevated). See ADR-141 §3 PR-W1.",
                        dasclaw_home.display()
                    ),
                });
            }
            // `EnterpriseGateOutcome` is `#[non_exhaustive]`; the wildcard
            // acts as a fail-safe contract guard if a future variant is
            // added without updating this matcher. After xClaw#446
            // (Wave-C1c P1.2b gate realignment) every OS sandbox kind
            // lands kernel enforcement, so the gate produces only `Allow`
            // or `DenyNoKernelSandbox` for Windows today.
            other => {
                return Err(SandboxError::PolicyTransform(format!(
                    "internal: check_enterprise_gate returned {other:?} for \
                     WindowsRestrictedToken; gate must Allow on Windows after \
                     ADR-141 §3 PR-W3 (PR #426) + xClaw#434 realignment."
                )));
            }
        }

        if !setup_complete {
            return Err(SandboxError::WindowsSetupPending {
                detail: format!(
                    "Windows sandbox not initialized at {}. Run \
                     dasclaw-sandbox-setup.exe (elevated) before invoking the sandbox.",
                    dasclaw_home.display()
                ),
            });
        }

        // 1. command → (program, args, cwd, env_map)
        let parts = decompose_command(&req.command)?;

        // 2. SandboxBackendConfig → upstream SandboxPolicy → JSON
        let upstream_policy = backend_config_to_upstream_policy(&req.policy)?;
        let policy_json = serde_json::to_string(&upstream_policy)
            .map_err(|e| SandboxError::PolicyTransform(format!("serialize policy: {e}")))?;

        // 3. ResourceLimits → outer Job Object wire form
        let outer_limits = build_outer_limits(&req.policy.resource_limits);

        // 4. spawn launcher binary（ADR-131 side-by-side wrapper）
        let request = LauncherRequest {
            protocol_version: PROTOCOL_VERSION,
            argv: parts.argv,
            cwd: parts.cwd,
            env: parts.env,
            dasclaw_home,
            policy_json,
            outer_limits,
            // use_private_desktop=false：维持 first-use UX 简单；Phase 1.3 / hardening 时再切换。
            use_private_desktop: false,
            // ADR-141 §3 PR-W3 / OQ-W3-2 (sign-off 2026-05-11)：把
            // `SandboxBackendConfig::read_only_subpaths` 透传给 launcher，
            // launcher 改调 upstream
            // `run_windows_sandbox_capture_with_extra_deny_write_paths`
            // 来下发 Win32 DACL DENY ACE（kernel-enforced read-only holes
            // inside writable workspaces）。空 Vec 时维持 Slice B1 行为。
            additional_deny_write_paths: req.policy.read_only_subpaths.clone(),
        };

        launcher_client::spawn_and_capture(request)
    }
}

/// 把 [`ResourceLimits`] 投影到 launcher IPC 的 [`OuterJobLimitsWire`]。
///
/// 全部字段为 `None` 时返回 `None`，让 launcher 跳过 outer Job Object 创建
/// （它仍会调 `run_windows_sandbox_capture`，沙箱内层自身的 inner Job Object
/// 由 `dasclaw_sandbox_windows` 上游负责）。
///
/// `max_processes` 是 `Option<u64>`，但 Windows `JOB_OBJECT_BASIC_LIMIT.ActiveProcessLimit`
/// 是 `u32`。`u64 → u32` 失败时静默丢弃该字段（保留其它限制），不让
/// 整个 outer 配置失败 —— 因为 4 GiB 内存上限仍然有意义。
fn build_outer_limits(rl: &ResourceLimits) -> Option<OuterJobLimitsWire> {
    let any = rl.max_memory_bytes.is_some() || rl.max_processes.is_some();
    if !any {
        return None;
    }
    Some(OuterJobLimitsWire {
        max_process_memory_bytes: rl.max_memory_bytes,
        max_job_memory_bytes: None,
        max_active_processes: rl.max_processes.and_then(|n| u32::try_from(n).ok()),
    })
}

/// Decomposed [`std::process::Command`] suitable for the upstream
/// `run_windows_sandbox_capture` API.
struct CommandParts {
    /// `argv[0]` is the program; `argv[1..]` are the arguments. Upstream
    /// re-quotes with `winutil::quote_windows_arg`.
    argv: Vec<String>,
    cwd: PathBuf,
    env: HashMap<String, String>,
}

fn decompose_command(cmd: &std::process::Command) -> Result<CommandParts, SandboxError> {
    let program = cmd
        .get_program()
        .to_str()
        .ok_or_else(|| SandboxError::PolicyTransform("program is not valid UTF-8".into()))?
        .to_string();

    let mut argv = Vec::with_capacity(1 + cmd.get_args().count());
    argv.push(program);
    for a in cmd.get_args() {
        let s = a
            .to_str()
            .ok_or_else(|| SandboxError::PolicyTransform("argument is not valid UTF-8".into()))?
            .to_string();
        argv.push(s);
    }

    let cwd = cmd
        .get_current_dir()
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| SandboxError::PolicyTransform("cannot resolve cwd for command".into()))?;

    let mut env: HashMap<String, String> = HashMap::new();
    for (k, v) in cmd.get_envs() {
        let key = k
            .to_str()
            .ok_or_else(|| SandboxError::PolicyTransform("env key is not valid UTF-8".into()))?
            .to_string();
        match v {
            Some(val) => {
                let val = val
                    .to_str()
                    .ok_or_else(|| {
                        SandboxError::PolicyTransform("env value is not valid UTF-8".into())
                    })?
                    .to_string();
                env.insert(key, val);
            }
            None => {
                // Command::env_remove — the upstream API is "what env to set",
                // there is no remove-from-inherited concept. Just skip.
                env.remove(&key);
            }
        }
    }

    Ok(CommandParts { argv, cwd, env })
}

/// Translate [`SandboxBackendConfig`] into upstream `windows_sandbox::SandboxPolicy`.
///
/// Pure / platform-agnostic so unit tests can exercise it on macOS too — but
/// gated under `cfg(target_os = "windows")` because [`AbsolutePathBuf`] only
/// type-checks once `dasclaw_sandbox_windows` is in scope.
fn backend_config_to_upstream_policy(
    cfg: &SandboxBackendConfig,
) -> Result<UpstreamPolicy, SandboxError> {
    let _ = NetworkAccess::Restricted; // silence unused-import-on-feature-axes; kept for clarity below.
    if cfg.writable_roots.is_empty() {
        Ok(UpstreamPolicy::ReadOnly {
            network_access: cfg.allow_network,
        })
    } else {
        let mut writable_roots = Vec::with_capacity(cfg.writable_roots.len());
        for p in &cfg.writable_roots {
            let abs = AbsolutePathBuf::from_absolute_path_checked(p).map_err(|e| {
                SandboxError::PolicyTransform(format!(
                    "writable_root '{}' is not absolute: {}",
                    p.display(),
                    e
                ))
            })?;
            writable_roots.push(abs);
        }
        Ok(UpstreamPolicy::WorkspaceWrite {
            writable_roots,
            network_access: cfg.allow_network,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        })
    }
}

/// 解析 dasclaw_home 路径。优先级见模块文档。
fn resolve_dasclaw_home() -> Result<PathBuf, SandboxError> {
    if let Ok(h) = std::env::var("DASCLAW_HOME") {
        if !h.is_empty() {
            return Ok(PathBuf::from(h));
        }
    }
    if let Ok(h) = std::env::var("CODEX_HOME") {
        if !h.is_empty() {
            return Ok(PathBuf::from(h));
        }
    }
    let base = dirs::config_dir()
        .ok_or_else(|| SandboxError::PolicyTransform("cannot resolve user config dir".into()))?;
    Ok(base.join("dasclaw"))
}

/// Backend for [`crate::sandbox_setup_status`] on Windows. Read-only
/// inspection of `dasclaw_home` + `sandbox_setup_is_complete()`. Does **not**
/// spawn the launcher or mutate any state — safe to call at process
/// startup (epic #380 / issue #464).
pub(crate) fn sandbox_setup_status_windows(kind: crate::SandboxType) -> crate::SandboxSetupStatus {
    use crate::SandboxSetupStatus;

    match resolve_dasclaw_home() {
        Ok(dasclaw_home) => {
            if sandbox_setup_is_complete(&dasclaw_home) {
                SandboxSetupStatus::Ready { kind }
            } else {
                SandboxSetupStatus::SetupRequired {
                    kind,
                    dasclaw_home,
                    action_hint: "Run dasclaw-sandbox-setup.exe (elevated). See \
                         desktop-client/docs/windows-sandbox-setup-guide.md."
                        .into(),
                }
            }
        }
        Err(e) => SandboxSetupStatus::SetupRequired {
            kind,
            dasclaw_home: PathBuf::from("<unresolved>"),
            action_hint: format!(
                "Cannot resolve dasclaw_home ({e}); set DASCLAW_HOME, then run \
                 dasclaw-sandbox-setup.exe (elevated)."
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_stable() {
        let sb = WindowsRestrictedTokenSandbox::new();
        assert_eq!(sb.kind(), SandboxType::WindowsRestrictedToken);
    }

    #[test]
    fn empty_writable_roots_yields_read_only() {
        let cfg = SandboxBackendConfig::default();
        let policy = backend_config_to_upstream_policy(&cfg).expect("ok");
        assert!(matches!(
            policy,
            UpstreamPolicy::ReadOnly {
                network_access: false
            }
        ));
    }

    #[test]
    fn allow_network_propagates_in_read_only() {
        let mut cfg = SandboxBackendConfig::default();
        cfg.allow_network = true;
        let policy = backend_config_to_upstream_policy(&cfg).expect("ok");
        assert!(matches!(
            policy,
            UpstreamPolicy::ReadOnly {
                network_access: true
            }
        ));
    }

    #[test]
    fn writable_root_yields_workspace_write() {
        // C:\ is the only universally-absolute root we can rely on in tests
        // running on Windows runners. Adapter unit tests are
        // `cfg(target_os = "windows")` gated together with the module.
        let cfg =
            SandboxBackendConfig::default().with_writable(PathBuf::from("C:\\Users\\TestUser"));
        let policy = backend_config_to_upstream_policy(&cfg).expect("ok");
        match policy {
            UpstreamPolicy::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => {
                assert_eq!(writable_roots.len(), 1);
                assert!(!network_access);
                assert!(!exclude_tmpdir_env_var);
                assert!(!exclude_slash_tmp);
            }
            other => panic!("expected WorkspaceWrite, got {other:?}"),
        }
    }

    #[test]
    fn relative_writable_root_is_rejected() {
        let cfg = SandboxBackendConfig::default().with_writable(PathBuf::from("relative\\path"));
        let err = backend_config_to_upstream_policy(&cfg).unwrap_err();
        assert!(matches!(err, SandboxError::PolicyTransform(_)));
    }

    #[test]
    fn dasclaw_home_prefers_env_var() {
        // SAFETY: tests touch process env; serial harness assumed via cfg(test)
        // unit-test isolation.
        // safety: test-only env mutation; restored in finally-style cleanup.
        unsafe {
            std::env::set_var("DASCLAW_HOME", "C:\\custom\\dasclaw");
        }
        let h = resolve_dasclaw_home().expect("ok");
        assert_eq!(h, PathBuf::from("C:\\custom\\dasclaw"));
        // safety: test-only env mutation cleanup.
        unsafe {
            std::env::remove_var("DASCLAW_HOME");
        }
    }
}
