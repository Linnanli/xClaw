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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dasclaw_sandbox_windows::absolute_path::AbsolutePathBuf;
use dasclaw_sandbox_windows::types::{NetworkAccess, SandboxPolicy as UpstreamPolicy};
use dasclaw_sandbox_windows::{run_windows_sandbox_capture, sandbox_setup_is_complete};

use crate::{Sandbox, SandboxBackendConfig, SandboxError, SandboxExecRequest, SandboxType};

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

        if !sandbox_setup_is_complete(&dasclaw_home) {
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

        // 3. dispatch
        let capture = run_windows_sandbox_capture(
            &policy_json,
            &parts.cwd,
            &dasclaw_home,
            parts.argv,
            &parts.cwd,
            parts.env,
            None,  // timeout — outer caller (OsExecutor) wraps with tokio timeout
            false, // use_private_desktop=false: keep first-use UX simple; flip in Phase 1.3 / hardening
        )
        .map_err(|e| {
            SandboxError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        // 4. CaptureResult → std::process::Output
        Ok(capture_result_to_output(capture))
    }
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

fn capture_result_to_output(
    capture: dasclaw_sandbox_windows::CaptureResult,
) -> std::process::Output {
    use std::os::windows::process::ExitStatusExt;

    let status = std::process::ExitStatus::from_raw(capture.exit_code as u32);
    std::process::Output {
        status,
        stdout: capture.stdout,
        stderr: capture.stderr,
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
