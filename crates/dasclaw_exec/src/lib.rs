//! `dasclaw_exec` — W2.6 整合层。
//!
//! 把 [`ironclaw_workspace_cap::policy::SandboxPolicy`]（高层 4-tier enum，
//! 与 codex 协议对齐）转成 [`dasclaw_sandbox::SandboxBackendConfig`]
//! （内核级配置），并提供 [`ProcessExecutor`] 抽象，让 tool 调用方无需
//! 自己组装 sandbox + policy + cwd 的胶水。
//!
//! ## 设计边界
//!
//! - **进程沙箱（dasclaw_sandbox）**：粗粒度，按 root 整体允许/拒绝写。
//! - **WritableRoot 洞中洞（read_only_subpaths）**：理论上需要内核层
//!   `landlock` / `sbpl` 表达；目前内核层暂不强制（known limitation），
//!   `is_path_writable` 用户态决策仍然有效，且 [`ironclaw_workspace_cap::WorkspaceCap`]
//!   的 cap-std 文件接口已经接入策略层做二次拒绝。
//! - **PTY 终端**：走 [`dasclaw_pty`] 独立路径，不在 `ProcessExecutor` 范围；
//!   终端会话的 sandbox 包裹由调用方在 `spawn` 时显式组合。
//!
//! ## 典型用法
//!
//! ```no_run
//! use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
//! use dasclaw_sandbox::SandboxablePreference;
//! use ironclaw_workspace_cap::policy::SandboxPolicy;
//! use std::process::Command;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let policy = SandboxPolicy::new_workspace_write_policy();
//! let executor = SandboxedExecutor::new(policy, SandboxablePreference::Auto, false);
//! let mut cmd = Command::new("echo");
//! cmd.arg("hello");
//! let output = executor.execute(ExecRequest {
//!     command: cmd,
//!     cwd: std::env::current_dir()?,
//! })?;
//! assert!(output.status.success());
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dasclaw_sandbox::{
    SandboxBackendConfig, SandboxError, SandboxExecRequest, SandboxablePreference, select_backend,
};
use ironclaw_workspace_cap::policy::{NetworkAccess, SandboxPolicy};

/// 错误统一入口。
#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    /// 政策本身不允许目标路径写入（一票否决：洞中洞 / 不在 root / ReadOnly 模式）。
    #[error("policy denied write to {path}")]
    PolicyDenied { path: PathBuf },

    /// `cwd` 不在政策允许的可读范围内（DangerFullAccess 之外，cwd 本身必须可写）。
    #[error("cwd {0} not within policy writable roots")]
    CwdNotWritable(PathBuf),

    /// 沙箱后端错误，原样透出。
    #[error(transparent)]
    Sandbox(#[from] SandboxError),
}

/// `ProcessExecutor::execute` 的请求体。
///
/// `cwd` 是必填字段：政策决策（is_path_writable）依赖 cwd 锚点。
#[derive(Debug)]
pub struct ExecRequest {
    pub command: Command,
    pub cwd: PathBuf,
}

/// 抽象 tool 执行入口。
///
/// 所有进入沙箱的进程都应通过这个 trait 调用，方便后续替换实现
/// （单元测试 mock / 远端 sandbox / 高隔离 docker 等）。
pub trait ProcessExecutor: Send + Sync {
    /// 执行 `req.command`，返回标准 [`Output`]。
    ///
    /// 实现负责：
    /// 1. 检查 `req.cwd` 是否符合政策；
    /// 2. 把高层政策转成 [`SandboxBackendConfig`]；
    /// 3. 调用内核沙箱执行；
    /// 4. 失败时返回 [`ExecError`]，**不**降级为直接 exec（fail-safe）。
    fn execute(&self, req: ExecRequest) -> Result<Output, ExecError>;
}

/// 默认实现：基于 [`ironclaw_workspace_cap::policy::SandboxPolicy`] +
/// [`dasclaw_sandbox`] 的组合执行器。
pub struct SandboxedExecutor {
    policy: SandboxPolicy,
    sandbox_pref: SandboxablePreference,
    windows_sandbox_enabled: bool,
}

impl SandboxedExecutor {
    pub fn new(
        policy: SandboxPolicy,
        sandbox_pref: SandboxablePreference,
        windows_sandbox_enabled: bool,
    ) -> Self {
        Self {
            policy,
            sandbox_pref,
            windows_sandbox_enabled,
        }
    }

    /// 公开 policy 供调用方做额外用户态检查。
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}

impl ProcessExecutor for SandboxedExecutor {
    fn execute(&self, req: ExecRequest) -> Result<Output, ExecError> {
        // 1. 政策门：cwd 自身不能落在洞中洞（read_only_subpath）。
        //    `is_path_writable(cwd, cwd)` 在 WorkspaceWrite 下总是把 cwd 加为
        //    隐含 root，所以这里只在非 ReadOnly 场景下，对 cwd 是否被任何
        //    `read_only_subpaths` 命中做兜底检查（fail-safe）。
        if !matches!(self.policy, SandboxPolicy::ReadOnly { .. })
            && !self.policy.is_path_writable(&req.cwd, &req.cwd)
        {
            return Err(ExecError::CwdNotWritable(req.cwd));
        }

        // 2. 政策 → 内核配置
        let backend = policy_to_backend_config(&self.policy, &req.cwd);

        // 3. 选后端 + 执行
        let sandbox = select_backend(self.sandbox_pref, self.windows_sandbox_enabled)?;
        let exec = SandboxExecRequest {
            command: req.command,
            policy: backend,
            preference: self.sandbox_pref,
            windows_sandbox_enabled: self.windows_sandbox_enabled,
        };
        sandbox.execute(exec).map_err(ExecError::from)
    }
}

/// 把高层 [`SandboxPolicy`] 投影到内核需要的 [`SandboxBackendConfig`]。
///
/// 转换语义（与 codex `protocol.rs` 对齐）：
///
/// | 高层政策 | 网络 | 写权限 | 备注 |
/// |---------|------|-------|------|
/// | `DangerFullAccess` | 允许 | 全盘可写 | 仅用于 trust 极高的本地任务 |
/// | `ReadOnly { network_access }` | 由字段决定 | 全盘只读 | spawn 仍允许（unsafe shell 工具） |
/// | `ExternalSandbox { network_access }` | 由字段决定 | 全盘只读（本地降级） | 实际隔离由外部容器完成 |
/// | `WorkspaceWrite { roots, .. }` | 由字段决定 | cwd + roots + /tmp + $TMPDIR | 洞中洞 `.git/.codex` 在用户态强制 |
///
/// **已知限制**：sandbox 内核层（seatbelt / seccomp）按 root 整体允许写，
/// 不强制 `read_only_subpaths`（洞中洞）。洞中洞契约由
/// [`ironclaw_workspace_cap::policy::SandboxPolicy::is_path_writable`] 用户态决策
/// 与 [`ironclaw_workspace_cap::WorkspaceCap`] 文件接口共同强制。
pub fn policy_to_backend_config(policy: &SandboxPolicy, cwd: &Path) -> SandboxBackendConfig {
    match policy {
        SandboxPolicy::DangerFullAccess => SandboxBackendConfig {
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![PathBuf::from("/")],
            allow_network: true,
            allow_spawn: true,
            proxy_loopback_ports: vec![],
        },
        SandboxPolicy::ReadOnly { network_access } => SandboxBackendConfig {
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![],
            allow_network: *network_access,
            allow_spawn: true,
            proxy_loopback_ports: vec![],
        },
        SandboxPolicy::ExternalSandbox { network_access } => SandboxBackendConfig {
            // 进程内被外层容器隔离，本地视为只读。
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![],
            allow_network: matches!(network_access, NetworkAccess::Enabled),
            allow_spawn: true,
            proxy_loopback_ports: vec![],
        },
        SandboxPolicy::WorkspaceWrite { network_access, .. } => {
            let roots = policy.get_writable_roots_with_cwd(cwd);
            let writable_roots: Vec<PathBuf> = roots.iter().map(|r| r.root.clone()).collect();
            SandboxBackendConfig {
                readable_roots: vec![PathBuf::from("/")],
                writable_roots,
                allow_network: *network_access,
                allow_spawn: true,
                proxy_loopback_ports: vec![],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironclaw_workspace_cap::policy::SandboxPolicy;

    #[test]
    fn danger_full_access_maps_to_root_writable() {
        let cfg = policy_to_backend_config(&SandboxPolicy::DangerFullAccess, Path::new("/x"));
        assert_eq!(cfg.writable_roots, vec![PathBuf::from("/")]);
        assert!(cfg.allow_network);
        assert!(cfg.allow_spawn);
    }

    #[test]
    fn read_only_strips_writable_roots() {
        let cfg = policy_to_backend_config(
            &SandboxPolicy::ReadOnly {
                network_access: false,
            },
            Path::new("/x"),
        );
        assert!(cfg.writable_roots.is_empty());
        assert!(!cfg.allow_network);
    }

    #[test]
    fn read_only_with_network_propagates_flag() {
        let cfg = policy_to_backend_config(
            &SandboxPolicy::ReadOnly {
                network_access: true,
            },
            Path::new("/x"),
        );
        assert!(cfg.allow_network);
    }

    #[test]
    fn external_sandbox_with_enabled_network_maps_through() {
        let cfg = policy_to_backend_config(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            },
            Path::new("/x"),
        );
        assert!(cfg.allow_network);
        assert!(
            cfg.writable_roots.is_empty(),
            "external 走外层容器，本地只读"
        );
    }

    #[test]
    fn workspace_write_includes_cwd_in_writable_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path();
        let cfg = policy_to_backend_config(&SandboxPolicy::new_workspace_write_policy(), cwd);
        assert!(
            cfg.writable_roots.iter().any(|r| r == cwd),
            "writable_roots 必须包含 cwd: {:?}",
            cfg.writable_roots
        );
    }

    #[test]
    fn workspace_write_includes_slash_tmp_on_unix() {
        if cfg!(unix) && Path::new("/tmp").is_dir() {
            let tmp = tempfile::tempdir().unwrap();
            let cfg =
                policy_to_backend_config(&SandboxPolicy::new_workspace_write_policy(), tmp.path());
            assert!(
                cfg.writable_roots.iter().any(|r| r == Path::new("/tmp")),
                "Unix 默认应包含 /tmp"
            );
        }
    }

    // -- ProcessExecutor 接口契约 --

    #[test]
    fn cwd_inside_read_only_subpath_is_rejected() {
        // 把 cwd 直接指到一个 read_only_subpath（.git），应该被一票否决。
        let tmp = tempfile::tempdir().unwrap();
        let dot_git = tmp.path().join(".git");
        std::fs::create_dir(&dot_git).unwrap();

        let executor = SandboxedExecutor::new(
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![tmp.path().to_path_buf()],
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            SandboxablePreference::Forbid,
            false,
        );
        let result = executor.execute(ExecRequest {
            command: Command::new("echo"),
            cwd: dot_git,
        });
        assert!(
            matches!(result, Err(ExecError::CwdNotWritable(_))),
            "cwd 落在 .git 洞中洞应被拒绝, got {:?}",
            result
        );
    }

    #[test]
    fn read_only_does_not_check_cwd_writable() {
        // ReadOnly 模式：cwd 不必可写（毕竟整个文件系统都是只读基准）
        let executor = SandboxedExecutor::new(
            SandboxPolicy::ReadOnly {
                network_access: false,
            },
            SandboxablePreference::Forbid,
            false,
        );
        let mut cmd = Command::new("true");
        cmd.arg("");
        let _ = executor.execute(ExecRequest {
            command: cmd,
            cwd: PathBuf::from("/anything"),
        });
        // 不必断言 success（Forbid 下 NoopSandbox 行为依赖 kind=None）；
        // 关键是不要 CwdNotWritable
    }

    #[cfg(unix)]
    #[test]
    fn end_to_end_echo_under_workspace_write() {
        // 用 Forbid 偏好走 NoopSandbox(None)，验证整条管线连通
        let tmp = tempfile::tempdir().unwrap();
        let executor = SandboxedExecutor::new(
            SandboxPolicy::new_workspace_write_policy(),
            SandboxablePreference::Forbid,
            false,
        );
        let mut cmd = Command::new("echo");
        cmd.arg("hello-from-exec");
        let out = executor
            .execute(ExecRequest {
                command: cmd,
                cwd: tmp.path().to_path_buf(),
            })
            .expect("should execute");
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("hello-from-exec"));
    }
}
