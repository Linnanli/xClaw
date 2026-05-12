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
//! - **WritableRoot 洞中洞（read_only_subpaths）**：自 ADR-135 §3 PR-C1（Wave-C1）
//!   起，macOS 上由 `dasclaw_sandbox` 委托给 `dasclaw_sandboxing::seatbelt` 在
//!   sbpl 中表达，sandbox-exec 内核层强制 `.git/`、`.dasclaw/`、`.codex/` 等
//!   敏感子路径仅读不可写；`is_path_writable` 用户态决策仍然作为第一道关，
//!   [`ironclaw_workspace_cap::WorkspaceCap`] 的 cap-std 文件接口在策略层做
//!   二次拒绝。Windows 上自 ADR-141 §3 PR-W3（Wave-C1b，PR #426）起由
//!   `dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths`
//!   把 `read_only_subpaths` 翻译为 Win32 DACL DENY entries，Windows 沙箱内核层
//!   强制相同语义。Linux landlock 洞中洞 deferred to Wave-C1c（需要
//!   `dasclaw-linux-sandbox` binary 落地）。
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
//! let executor = SandboxedExecutor::new(policy, SandboxablePreference::Auto, false, None);
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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use dasclaw_net_proxy::NetworkProxy;
use dasclaw_sandbox::proxy::detect_loopback_ports;
use dasclaw_sandbox::{
    ResourceLimits, SandboxBackendConfig, SandboxError, SandboxExecRequest, SandboxablePreference,
    select_backend,
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
    /// 会话级 [`NetworkProxy`]，构造期注入。macOS Seatbelt 后端将其转译为
    /// `(allow network-outbound (remote ip "localhost:<port>"))` sbpl 规则，
    /// 让沙箱内子进程的 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量真正生效
    /// （ADR-135 §3 PR-C2b / ADR-142）。
    ///
    /// `None` 等价于"无 proxy 注入"——backend 不会合成任何 loopback hole；
    /// 这是 fail-safe 的缺省值（Wave-C2a 在 dasclaw_exec 出口处使用过的
    /// 占位语义现在由调用方显式选择）。Linux 后端继续走 `backend.proxy_loopback_ports`
    /// 的 env-detect 路径，与 `network` 互不冲突。
    network: Option<NetworkProxy>,

    /// 路径到 `dasclaw-sandbox-linux` helper 二进制。Linux 内核层 FS 隔离
    /// （bubblewrap + landlock）必须经此 helper；ADR-144 §5.3 P1.2a wiring。
    /// macOS / Windows 后端忽略；Linux 上为 `None` ⇒ adapter fail-closed。
    ///
    /// 调用方（如 desktop-client `OsExecutor`）在启动时解析
    /// `std::env::current_exe()?.parent()?.join("dasclaw-sandbox-linux")`，
    /// 通过 [`Self::with_linux_sandbox_exe`] 注入。
    linux_sandbox_exe: Option<PathBuf>,
}

impl SandboxedExecutor {
    pub fn new(
        policy: SandboxPolicy,
        sandbox_pref: SandboxablePreference,
        windows_sandbox_enabled: bool,
        network: Option<NetworkProxy>,
    ) -> Self {
        Self {
            policy,
            sandbox_pref,
            windows_sandbox_enabled,
            network,
            linux_sandbox_exe: None,
        }
    }

    /// 配置 `dasclaw-sandbox-linux` helper 二进制路径（ADR-144 §5.3 P1.2a）。
    ///
    /// 此值会被透传到 [`SandboxBackendConfig::linux_sandbox_exe`]，Linux 后端
    /// 依据它决定走 helper 包装路径还是 fail-closed。macOS / Windows 上忽略。
    pub fn with_linux_sandbox_exe(mut self, exe: Option<PathBuf>) -> Self {
        self.linux_sandbox_exe = exe;
        self
    }

    /// 公开 policy 供调用方做额外用户态检查。
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    /// 公开会话级 [`NetworkProxy`] 引用，便于调用方 / 测试断言注入是否到位。
    pub fn network(&self) -> Option<&NetworkProxy> {
        self.network.as_ref()
    }

    /// 把 [`ExecRequest`] 翻译成 [`SandboxExecRequest`]，**不执行**。
    ///
    /// 抽出来供契约测试断言 `network` 是否被忠实透传，同时让 `execute()`
    /// 保持单一职责（assemble → run）。语义错误（cwd 落入洞中洞等）在这里
    /// 一票否决；通过后返回的 `SandboxExecRequest` 即喂给 backend.execute()
    /// 的最终形态。
    pub fn assemble_request(&self, req: ExecRequest) -> Result<SandboxExecRequest, ExecError> {
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
        let mut backend = policy_to_backend_config(&self.policy, &req.cwd);
        // ADR-144 §5.3 P1.2a：透传 helper exe，让 Linux 后端 wiring。
        backend.linux_sandbox_exe = self.linux_sandbox_exe.clone();

        Ok(SandboxExecRequest {
            command: req.command,
            policy: backend,
            preference: self.sandbox_pref,
            windows_sandbox_enabled: self.windows_sandbox_enabled,
            network: self.network.clone(),
        })
    }
}

impl ProcessExecutor for SandboxedExecutor {
    fn execute(&self, req: ExecRequest) -> Result<Output, ExecError> {
        let exec = self.assemble_request(req)?;
        let sandbox = select_backend(self.sandbox_pref, self.windows_sandbox_enabled)?;
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
/// | `WorkspaceWrite { roots, .. }` | 由字段决定 | cwd + roots + /tmp + $TMPDIR | 洞中洞 `.git/.codex/.dasclaw` 按平台分层强制（见下） |
///
/// **WritableRoot 洞中洞强制按平台分层**（与模块级 doc 一致）：
///
/// | 平台 | 内核层（kernel-enforced） | 用户态兜底 |
/// |------|---------------------------|-----------|
/// | macOS | ✅ 通过 `dasclaw_sandboxing::seatbelt` 在 sbpl 中表达 `(deny file-write* (subpath ".git/.codex/.dasclaw"))`（ADR-135 §3 PR-C1 / Wave-C1a） | [`ironclaw_workspace_cap`] cap-std + `is_path_writable` 第一道关 |
/// | Windows | ✅ 通过 `dasclaw_sandbox_windows::run_windows_sandbox_capture_with_extra_deny_write_paths` 把 `read_only_subpaths` 翻译为 Win32 DACL DENY entries（ADR-141 §3 PR-W3 / Wave-C1b，PR #426） | 同上 |
/// | Linux | ❌ deferred to Wave-C1c（需 `dasclaw-linux-sandbox` setuid binary 落地，对齐 codex landlock V5 + bwrap 路径） | 同上 |
///
/// Linux 上洞中洞当前仅靠 `is_path_writable` 用户态决策 +
/// [`ironclaw_workspace_cap::WorkspaceCap`] cap-std 文件接口拦截；子进程通过
/// 直接 syscall 写 `.git/hooks/` 在 Linux 上**目前不会被内核拒绝**。
/// 进度追踪：[issue #380](https://github.com/Linnanli/xClaw/issues/380) Wave-C1c。
pub fn policy_to_backend_config(policy: &SandboxPolicy, cwd: &Path) -> SandboxBackendConfig {
    policy_to_backend_config_with_env(policy, cwd, &std::env::vars().collect())
}

/// 可注入 env 的测试友好版本。`env` 应为完整进程环境变量集（
/// `std::env::vars().collect::<HashMap<_, _>>()`）；proxy 端口从这里提取。
///
/// 只在网络受限（`network_access=false` 或 `Restricted`）且检测到代理时才
/// 填充 `proxy_loopback_ports`；网络全开时 hole punch 多余，不填。
pub fn policy_to_backend_config_with_env(
    policy: &SandboxPolicy,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> SandboxBackendConfig {
    let proxy_ports_when_restricted = || detect_loopback_ports(env);

    match policy {
        SandboxPolicy::DangerFullAccess => SandboxBackendConfig {
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![PathBuf::from("/")],
            // FullAccess: no holes — full root is writable.
            read_only_subpaths: vec![],
            allow_network: true,
            allow_spawn: true,
            proxy_loopback_ports: vec![],
            // FullAccess is the user-trusted escape hatch; do not impose
            // resource limits here. Callers wanting limits on FullAccess
            // tasks must override `resource_limits` explicitly.
            resource_limits: ResourceLimits::unlimited(),
            enterprise_mode: false,
            linux_sandbox_exe: None,
        },
        SandboxPolicy::ReadOnly { network_access } => SandboxBackendConfig {
            readable_roots: vec![PathBuf::from("/")],
            writable_roots: vec![],
            // ReadOnly: no writable roots ⇒ no carve-outs needed.
            read_only_subpaths: vec![],
            allow_network: *network_access,
            allow_spawn: true,
            proxy_loopback_ports: if *network_access {
                vec![]
            } else {
                proxy_ports_when_restricted()
            },
            resource_limits: ResourceLimits::default(),
            enterprise_mode: false,
            linux_sandbox_exe: None,
        },
        SandboxPolicy::ExternalSandbox { network_access } => {
            let net_enabled = matches!(network_access, NetworkAccess::Enabled);
            SandboxBackendConfig {
                // 进程内被外层容器隔离，本地视为只读。
                readable_roots: vec![PathBuf::from("/")],
                writable_roots: vec![],
                // ExternalSandbox：内层视为只读，无 carve-outs。
                read_only_subpaths: vec![],
                allow_network: net_enabled,
                allow_spawn: true,
                proxy_loopback_ports: if net_enabled {
                    vec![]
                } else {
                    proxy_ports_when_restricted()
                },
                resource_limits: ResourceLimits::default(),
                enterprise_mode: false,
                linux_sandbox_exe: None,
            }
        }
        SandboxPolicy::WorkspaceWrite { network_access, .. } => {
            let roots = policy.get_writable_roots_with_cwd(cwd);
            let writable_roots: Vec<PathBuf> = roots.iter().map(|r| r.root.clone()).collect();
            // ADR-141 §3 PR-W3 / OQ-W3-2 (sign-off 2026-05-11)：flatten
            // 每个 WritableRoot 的 `read_only_subpaths` (例如 `.git/`,
            // `.codex/`, `.dasclaw/`) 到 backend config 顶层 —— 下游 Windows
            // adapter 会把它们 wire 给 launcher，再透传给上游
            // `run_windows_sandbox_capture_with_extra_deny_write_paths` 下发
            // Win32 DACL DENY ACE，达成 kernel-enforced 洞中洞。macOS sbpl
            // 由 ADR-135 PR-C1 在 `dasclaw_sandboxing::seatbelt` 处理，
            // Linux Landlock 是 PR-W4 / Wave-C1c 的工作。
            let read_only_subpaths: Vec<PathBuf> = roots
                .iter()
                .flat_map(|r| r.read_only_subpaths.iter().cloned())
                .collect();
            SandboxBackendConfig {
                readable_roots: vec![PathBuf::from("/")],
                writable_roots,
                read_only_subpaths,
                allow_network: *network_access,
                allow_spawn: true,
                proxy_loopback_ports: if *network_access {
                    vec![]
                } else {
                    proxy_ports_when_restricted()
                },
                resource_limits: ResourceLimits::default(),
                enterprise_mode: false,
                linux_sandbox_exe: None,
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

    // -- W3.3-1: ResourceLimits projection --

    #[test]
    fn danger_full_access_uses_unlimited_resources() {
        let cfg = policy_to_backend_config(&SandboxPolicy::DangerFullAccess, Path::new("/x"));
        // FullAccess is the user-trusted escape hatch; do not impose
        // resource limits unless the caller overrides explicitly.
        assert!(cfg.resource_limits.max_memory_bytes.is_none());
        assert!(cfg.resource_limits.max_cpu_secs.is_none());
    }

    #[test]
    fn read_only_uses_default_resource_limits() {
        let cfg = policy_to_backend_config(
            &SandboxPolicy::ReadOnly {
                network_access: false,
            },
            Path::new("/x"),
        );
        assert_eq!(
            cfg.resource_limits.max_memory_bytes,
            Some(4 * 1024 * 1024 * 1024)
        );
        assert_eq!(cfg.resource_limits.max_cpu_secs, Some(600));
    }

    #[test]
    fn workspace_write_uses_default_resource_limits() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg =
            policy_to_backend_config(&SandboxPolicy::new_workspace_write_policy(), tmp.path());
        assert!(cfg.resource_limits.max_memory_bytes.is_some());
        assert_eq!(cfg.resource_limits.max_open_files, Some(1024));
        assert_eq!(cfg.resource_limits.max_processes, Some(1024));
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
    fn req_policy_to_backend_config_flattens_read_only_subpaths() {
        // ADR-141 §3 PR-W3 / OQ-W3-2 (sign-off 2026-05-11):
        // SandboxPolicy::WorkspaceWrite 在 `get_writable_roots_with_cwd`
        // 里默认会给 cwd 加 `.git` / `.codex` / `.dasclaw` 等 read-only
        // subpaths（见 `default_read_only_subpaths_for_writable_root`）。
        // 本测试锁定: backend config 的 `read_only_subpaths` 必须把所有
        // root 的 holes flatten 到顶层 —— 否则 Windows adapter 无法把
        // 它们 wire 到 launcher IPC，最终 Win32 DACL DENY 会漏 ACE。
        let tmp = tempfile::tempdir().unwrap();
        let git_dir = tmp.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        let cfg =
            policy_to_backend_config(&SandboxPolicy::new_workspace_write_policy(), tmp.path());
        assert!(
            cfg.read_only_subpaths.iter().any(|p| p == &git_dir),
            "flatten 必须包含 cwd/.git，实际: {:?}",
            cfg.read_only_subpaths,
        );
    }

    #[test]
    fn req_read_only_policies_have_empty_read_only_subpaths() {
        // ReadOnly / ExternalSandbox / DangerFullAccess 没有 writable
        // carve-outs，read_only_subpaths 必须空。否则 ADR-141 gate
        // (Step 2) 误判为 needs-kernel-enforcement。
        for p in [
            SandboxPolicy::ReadOnly {
                network_access: false,
            },
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            SandboxPolicy::DangerFullAccess,
        ] {
            let cfg = policy_to_backend_config(&p, Path::new("/tmp"));
            assert!(
                cfg.read_only_subpaths.is_empty(),
                "{p:?} 不应产生 read_only_subpaths，实际: {:?}",
                cfg.read_only_subpaths,
            );
        }
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

    // -- W2.3b proxy 集成 --

    fn proxy_env(url: &str) -> HashMap<String, String> {
        let mut e = HashMap::new();
        e.insert("HTTPS_PROXY".to_string(), url.to_string());
        e
    }

    #[test]
    fn workspace_write_with_proxy_env_populates_proxy_ports() {
        let tmp = tempfile::tempdir().unwrap();
        let env = proxy_env("http://127.0.0.1:8080");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::new_workspace_write_policy(),
            tmp.path(),
            &env,
        );
        assert_eq!(
            cfg.proxy_loopback_ports,
            vec![8080],
            "网络受限 + 检测到 loopback proxy 应填 hole-punch 端口"
        );
        assert!(!cfg.allow_network, "WorkspaceWrite 默认 network=false");
    }

    #[test]
    fn workspace_write_with_network_enabled_skips_proxy_ports() {
        let tmp = tempfile::tempdir().unwrap();
        let env = proxy_env("http://127.0.0.1:8080");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            tmp.path(),
            &env,
        );
        assert!(
            cfg.proxy_loopback_ports.is_empty(),
            "网络全开时 hole-punch 多余，不应填 proxy 端口"
        );
        assert!(cfg.allow_network);
    }

    #[test]
    fn read_only_with_proxy_env_populates_proxy_ports_when_network_denied() {
        let env = proxy_env("socks5://localhost:1080");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::ReadOnly {
                network_access: false,
            },
            Path::new("/x"),
            &env,
        );
        assert_eq!(cfg.proxy_loopback_ports, vec![1080]);
    }

    #[test]
    fn external_sandbox_restricted_with_proxy_env_picks_up_ports() {
        let env = proxy_env("http://127.0.0.1:9999");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            Path::new("/x"),
            &env,
        );
        assert_eq!(cfg.proxy_loopback_ports, vec![9999]);
        assert!(!cfg.allow_network);
    }

    #[test]
    fn external_sandbox_enabled_skips_proxy_ports() {
        let env = proxy_env("http://127.0.0.1:9999");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            },
            Path::new("/x"),
            &env,
        );
        assert!(cfg.proxy_loopback_ports.is_empty());
        assert!(cfg.allow_network);
    }

    #[test]
    fn non_loopback_proxy_does_not_populate_ports() {
        // 真实远程 proxy（不是 loopback）→ sandbox 不该 hole-punch loopback
        let env = proxy_env("http://corp-proxy.internal:3128");
        let cfg = policy_to_backend_config_with_env(
            &SandboxPolicy::new_workspace_write_policy(),
            Path::new("/x"),
            &env,
        );
        assert!(
            cfg.proxy_loopback_ports.is_empty(),
            "非 loopback proxy 不应触发 hole-punch"
        );
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
            None,
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
            None,
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
            None,
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

    // NetworkProxy 透传契约 (ADR-135 §3 PR-C2b) 由集成测试覆盖：
    // crates/dasclaw_exec/tests/network_proxy_threading.rs
    // 该测试需要 dasclaw_net_proxy 公共 API 构造 unmanaged NetworkProxy，
    // 故走 tests/ 目录而非 #[cfg(test)] mod。
}
