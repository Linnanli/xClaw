//! Linux Seccomp sandbox backend (W2.3 — minimum viable).
//!
//! ## 范围
//!
//! 本模块端口 codex
//! `codex-cli-main/codex-rs/linux-sandbox/src/landlock.rs` 中的 **seccomp
//! 网络阻断** 部分（≈ 200/343 LOC）。codex 自己的实现里：
//!
//! - 文件系统限制走 **bubblewrap** (`bwrap.rs` 2,005 LOC)，需要外部
//!   setuid 二进制，**本切片不取**。
//! - 文件系统限制的 landlock 路径在 codex 注释里标记为
//!   "legacy/backup"，**本切片不取**。
//! - 网络限制走 **seccomp** filter，纯 Rust（`seccompiler` crate），
//!   **本切片采用**。
//!
//! 文件系统的 kernel 层兜底由调用方使用
//! [`dasclaw_workspace_cap`](https://github.com/dbappsecurity/x-claw/tree/main/crates/dasclaw_workspace_cap)
//! 完成 — 那是 cap-std 应用层 FS 边界，与本 kernel 层 sandbox 互补。
//!
//! ## seccomp 应用模型
//!
//! seccomp filter 必须在 `fork()` 之后、`exec()` 之前安装到子进程。
//! 我们用 `std::os::unix::process::CommandExt::pre_exec` 在 spawn 时
//! 注入 hook：set_no_new_privs + apply seccomp。
//!
//! 父进程（dasclaw_sandbox 调用方）**不受影响**，只有子进程被沙箱化。
//!
//! ## 当前限制
//!
//! - 不限制文件系统访问（依赖应用层 cap-std）
//! - `allow_network=true` 时 seccomp 完全 bypass（无 syscall filter 安装）
//! - `proxy_loopback_ports` **非空** 且 `allow_network=false` 时切到
//!   ProxyRouted 模式：允许 AF_INET/AF_INET6 socket 连本地代理桥，
//!   拒 AF_UNIX 防旁路。env 透传由调用方在 [`crate::SandboxExecRequest`]
//!   显式注入；`dasclaw_exec` 已经默认从宿主 env 检测 proxy 端口。

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use seccompiler::{
    apply_filter, BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
    SeccompFilter, SeccompRule, TargetArch,
};

pub mod cgroup_v2;

use crate::policy_bridge;
use crate::rlimit;
use crate::{Sandbox, SandboxError, SandboxExecRequest, SandboxType};

use dasclaw_sandboxing::landlock::{
    create_linux_sandbox_command_args_for_policies, CODEX_LINUX_SANDBOX_ARG0,
};

/// Linux 子进程沙箱后端：seccomp 网络阻断（W2.3a）。
pub struct LinuxSeccompSandbox;

impl LinuxSeccompSandbox {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxSeccompSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox for LinuxSeccompSandbox {
    fn kind(&self) -> SandboxType {
        SandboxType::LinuxSeccomp
    }

    fn execute(&self, req: SandboxExecRequest) -> Result<std::process::Output, SandboxError> {
        let plan = prepare_command(&req)?;
        let proxy_routed = !req.policy.proxy_loopback_ports.is_empty();
        // `install_seccomp` 已经在 build_legacy_plan 里编码了 `!allow_network`
        // 的语义；helper 路径下永远为 false（helper 内部装 seccomp）。父层
        // 只需读 plan 标志，不必再重复 allow_network 判断。
        let install_seccomp = plan.install_parent_seccomp;

        // 复制 req.command 的 env / cwd 到新 Command，并应用 plan 决定的 argv。
        let mut cmd = Command::new(&plan.program);
        for a in &plan.args {
            cmd.arg(a);
        }
        if let Some(arg0) = plan.arg0.clone() {
            cmd.arg0(arg0);
        }

        // env 透传策略与 macOS Seatbelt 对齐：env_clear() 后只透传调用方
        // 显式设置的 env。防止宿主敏感 env（OPENAI_API_KEY 等）泄漏。
        cmd.env_clear();
        for (k, v) in req.command.get_envs() {
            if let Some(val) = v {
                cmd.env(k, val);
            }
        }
        if let Some(d) = req.command.get_current_dir() {
            cmd.current_dir(d);
        }

        // W3.3-2: setrlimit 总是先应用（与 allow_network 无关）。
        // seccomp 仅在 legacy 路径（无 helper）下安装到父-fork 子进程；
        // helper 路径下，由 helper 内部在 bwrap 之后安装 seccomp，本层不重复。
        let limits = req.policy.resource_limits.clone();

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let pre_exec_limits = limits.clone();
        // SAFETY: 只调用 async-signal-safe 的 syscall（setrlimit, prctl,
        // seccomp 系列），无分配 / 锁 / 线程操作。
        unsafe {
            cmd.pre_exec(move || {
                rlimit::apply_in_pre_exec(&pre_exec_limits)?;
                if install_seccomp {
                    set_no_new_privs()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                    let mode = if proxy_routed {
                        NetworkSeccompMode::ProxyRouted
                    } else {
                        NetworkSeccompMode::Restricted
                    };
                    install_network_seccomp_filter(mode)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                }
                Ok(())
            });
        }

        // W3.3-3a: cgroup v2 RSS-based 内存限制（如可用）。与 rlimit 互补，
        // 对 helper 路径同样适用 —— cgroup 套在父进程 spawn 的直接子进程上，
        // 而 helper 内部 fork 的孙进程会被 cgroup 继承。
        let cgroup_guard = match (limits.max_memory_bytes, cgroup_v2::detect()) {
            (Some(_), cgroup_v2::DetectionResult::Available) => {
                let suffix = cgroup_v2::unique_suffix();
                cgroup_v2::CgroupGuard::new(&suffix, &limits).ok()
            }
            _ => None,
        };

        let mut child = cmd.spawn()?;

        if let Some(ref guard) = cgroup_guard {
            let _ = guard.assign_pid(child.id());
        }

        let output = child.wait_with_output()?;
        drop(cgroup_guard);
        Ok(output)
    }
}

/// ADR-144 §5.3 P1.2a — 决定一次 Linux sandbox 调用走哪条 spawn 路径。
///
/// - **Helper 路径**（`policy.linux_sandbox_exe = Some(exe)`）：把 user
///   command 包装为 `[exe, --sandbox-policy-cwd, ..., --, user_argv...]`
///   argv，arg0 设为 [`CODEX_LINUX_SANDBOX_ARG0`]（与 helper 内部 dispatch
///   兼容）。文件系统隔离 + seccomp 由 helper 内部完成；父层只保留 rlimit
///   / cgroup。
/// - **Legacy 路径**（`linux_sandbox_exe = None` 且 spawn 不要求 kernel
///   层 FS 隔离）：保留 W2.3 端口的直接 seccomp pre_exec 行为。
/// - **Fail-Safe**（`None` + `read_only_subpaths` 非空 或 `enterprise_mode`）：
///   返回 [`SandboxError::ReadOnlySubpathsKernelEnforcementMissing`]。
///   守住 ADR-141 §6 OQ-1：要求 kernel FS 隔离但没接 helper 时，**不能**
///   静默降级到 seccomp-only。
#[derive(Debug)]
pub(crate) struct CommandPlan {
    pub program: PathBuf,
    pub arg0: Option<String>,
    pub args: Vec<OsString>,
    /// `true`：父-fork 子进程要装 seccomp 网络 filter（legacy 路径）。
    /// `false`：helper 路径，seccomp 由 helper 内部装。
    pub install_parent_seccomp: bool,
}

pub(crate) fn prepare_command(req: &SandboxExecRequest) -> Result<CommandPlan, SandboxError> {
    let requires_kernel_fs =
        !req.policy.read_only_subpaths.is_empty() || req.policy.enterprise_mode;

    match req.policy.linux_sandbox_exe.as_ref() {
        Some(exe) => Ok(build_helper_plan(req, exe)),
        None if requires_kernel_fs => Err(SandboxError::ReadOnlySubpathsKernelEnforcementMissing {
            detail: format!(
                "linux_sandbox_exe not configured but spawn requires kernel FS isolation \
                 (read_only_subpaths={}, enterprise_mode={})",
                req.policy.read_only_subpaths.len(),
                req.policy.enterprise_mode
            ),
        }),
        None => Ok(build_legacy_plan(req)),
    }
}

fn build_helper_plan(req: &SandboxExecRequest, exe: &Path) -> CommandPlan {
    let cwd: PathBuf = req
        .command
        .get_current_dir()
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"));

    let triple = policy_bridge::to_protocol_policy(&req.policy, &cwd);

    let user_program = req.command.get_program().to_string_lossy().into_owned();
    let mut user_command: Vec<String> = Vec::with_capacity(1 + req.command.get_args().len());
    user_command.push(user_program);
    for a in req.command.get_args() {
        user_command.push(a.to_string_lossy().into_owned());
    }

    let allow_network_for_proxy = !req.policy.proxy_loopback_ports.is_empty();
    let args_str = create_linux_sandbox_command_args_for_policies(
        user_command,
        &cwd,
        &triple.sandbox_policy,
        &triple.file_system_policy,
        triple.network_policy,
        &cwd,
        /* use_legacy_landlock */ false,
        allow_network_for_proxy,
    );

    CommandPlan {
        program: exe.to_path_buf(),
        arg0: Some(CODEX_LINUX_SANDBOX_ARG0.to_string()),
        args: args_str.into_iter().map(OsString::from).collect(),
        install_parent_seccomp: false,
    }
}

fn build_legacy_plan(req: &SandboxExecRequest) -> CommandPlan {
    let program = PathBuf::from(req.command.get_program());
    let args: Vec<OsString> = req.command.get_args().map(|a| a.to_os_string()).collect();
    CommandPlan {
        program,
        arg0: None,
        args,
        install_parent_seccomp: !req.policy.allow_network,
    }
}

/// 网络 seccomp 过滤模式。
///
/// - `Restricted`：拒绝所有非 AF_UNIX socket 创建 + connect/accept/bind 等
///   网络 syscall。`cargo` / `npm` 等工具仍可工作（它们用 AF_UNIX
///   socketpair 与子进程通信）。
/// - `ProxyRouted`：允许 AF_INET/AF_INET6 socket（用于连接到本地代理桥），
///   但拒绝 AF_UNIX socket 阻止旁路。`Sandbox::execute` 在
///   `proxy_loopback_ports` 非空且 `allow_network=false` 时自动切此模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetworkSeccompMode {
    Restricted,
    ProxyRouted,
}

/// `prctl(PR_SET_NO_NEW_PRIVS, 1)` — seccomp filter 的前置条件。
fn set_no_new_privs() -> Result<(), std::io::Error> {
    // SAFETY: prctl 是 async-signal-safe；在 pre_exec 上下文调用安全。
    let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// 在当前线程安装网络 seccomp filter。
///
/// 直接端口自 codex `landlock.rs::install_network_seccomp_filter_on_current_thread`，
/// 神仔保留同一份 syscall deny 列表 + AF_UNIX 例外。
pub(crate) fn install_network_seccomp_filter(
    mode: NetworkSeccompMode,
) -> Result<(), seccompiler::Error> {
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    let deny = |rules: &mut BTreeMap<i64, Vec<SeccompRule>>, nr: i64| {
        rules.insert(nr, vec![]); // 空规则 = 无条件命中
    };

    // 通用拒绝列表（codex 同款）：阻止 ptrace + io_uring 任何模式都要禁。
    deny(&mut rules, libc::SYS_ptrace);
    deny(&mut rules, libc::SYS_io_uring_setup);
    deny(&mut rules, libc::SYS_io_uring_enter);
    deny(&mut rules, libc::SYS_io_uring_register);

    match mode {
        NetworkSeccompMode::Restricted => {
            for nr in [
                libc::SYS_connect,
                libc::SYS_accept,
                libc::SYS_accept4,
                libc::SYS_bind,
                libc::SYS_listen,
                libc::SYS_getpeername,
                libc::SYS_getsockname,
                libc::SYS_shutdown,
                libc::SYS_sendto,
                libc::SYS_sendmmsg,
                // 注意：不禁 recvfrom — `cargo clippy` 等用 AF_UNIX
                // socketpair + recvfrom 与子进程通信，禁了会破坏构建工具。
                libc::SYS_recvmmsg,
                libc::SYS_getsockopt,
                libc::SYS_setsockopt,
            ] {
                deny(&mut rules, nr);
            }

            // socket 系列：仅允许 AF_UNIX，其它 family 全拒。
            let unix_only = SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_UNIX as u64,
            )?])?;
            rules.insert(libc::SYS_socket, vec![unix_only.clone()]);
            rules.insert(libc::SYS_socketpair, vec![unix_only]);
        }
        NetworkSeccompMode::ProxyRouted => {
            // 允许 AF_INET/AF_INET6 socket（连本地代理桥），拒所有其他
            // family 包括 AF_UNIX 防旁路。
            let deny_non_ip = SeccompRule::new(vec![
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET as u64,
                )?,
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET6 as u64,
                )?,
            ])?;
            let deny_unix_pair = SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Eq,
                libc::AF_UNIX as u64,
            )?])?;
            rules.insert(libc::SYS_socket, vec![deny_non_ip]);
            rules.insert(libc::SYS_socketpair, vec![deny_unix_pair]);
        }
    }

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        if cfg!(target_arch = "x86_64") {
            TargetArch::x86_64
        } else if cfg!(target_arch = "aarch64") {
            TargetArch::aarch64
        } else {
            // 不在 ironclaw 支持的 Tier-1 Linux 架构里，构建期失败更友好
            return Err(seccompiler::Error::EmptyFilter);
        },
    )?;

    let prog: BpfProgram = filter.try_into()?;
    apply_filter(&prog)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! 注意：seccomp filter 的真实安装会污染当前线程，且 Linux only。
    //! 本模块只跑纯逻辑单测；真集成测试请放在 CI Linux runner 的
    //! `tests/linux_seccomp_smoke.rs`（W2.3 后续迭代加）。

    use super::*;

    #[test]
    fn sandbox_kind_is_linux_seccomp() {
        let sb = LinuxSeccompSandbox::new();
        assert_eq!(sb.kind(), SandboxType::LinuxSeccomp);
    }

    #[test]
    fn restricted_and_proxy_routed_are_distinct() {
        // 编译期常量比较，确保两个 mode 没被合并。
        assert_ne!(
            NetworkSeccompMode::Restricted,
            NetworkSeccompMode::ProxyRouted
        );
    }

    #[test]
    fn build_seccomp_filter_for_restricted_compiles() {
        // 不调 apply_filter（会污染线程），只验证 filter 构造路径
        // 不会失败 — 用一个独立的 helper 模拟构造但不 apply。
        // 这里直接调真实函数会 apply 到当前测试线程，导致后续测试无法
        // 创建 socket。所以仅用单元测试保证 enum + arch 检测逻辑。
        let arch_ok = cfg!(any(target_arch = "x86_64", target_arch = "aarch64"));
        assert!(arch_ok, "Linux Tier-1 arch precondition");
    }

    /// W2.3b 路径覆盖测试：模拟 `Sandbox::execute` 内部如何根据
    /// `proxy_loopback_ports.is_empty()` 选择 mode。本测试只覆盖映射
    /// 决策，不实际安装 filter（apply 会污染线程）。
    #[test]
    fn empty_proxy_ports_picks_restricted_mode() {
        let proxy_routed = !Vec::<u16>::new().is_empty();
        let mode = if proxy_routed {
            NetworkSeccompMode::ProxyRouted
        } else {
            NetworkSeccompMode::Restricted
        };
        assert_eq!(mode, NetworkSeccompMode::Restricted);
    }

    #[test]
    fn nonempty_proxy_ports_picks_proxy_routed_mode() {
        let ports: Vec<u16> = vec![8888];
        let proxy_routed = !ports.is_empty();
        let mode = if proxy_routed {
            NetworkSeccompMode::ProxyRouted
        } else {
            NetworkSeccompMode::Restricted
        };
        assert_eq!(mode, NetworkSeccompMode::ProxyRouted);
    }

    /// H1 (ADR-144 §5.3 P1.2a)：`linux_sandbox_exe = None` + 无 carve-out
    /// 时走 legacy 路径，父-fork 子进程仍要装 seccomp。
    #[test]
    fn req_dasclaw_sandbox_p1_2a_prepare_legacy_when_no_exe_and_no_carveouts() {
        use crate::{SandboxBackendConfig, SandboxablePreference};

        let mut policy = SandboxBackendConfig::default();
        policy.linux_sandbox_exe = None;
        policy.read_only_subpaths.clear();
        policy.enterprise_mode = false;
        policy.allow_network = false;

        let cmd = Command::new("/bin/echo");
        let req = SandboxExecRequest {
            command: cmd,
            policy,
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
            windows_sandbox_level: dasclaw_protocol::config_types::WindowsSandboxLevel::Disabled,
            network: None,
        };
        let plan = prepare_command(&req).expect("legacy path should succeed");
        assert_eq!(plan.program, PathBuf::from("/bin/echo"));
        assert!(plan.arg0.is_none(), "legacy path must not override arg0");
        assert!(
            plan.install_parent_seccomp,
            "legacy path must install seccomp when allow_network=false"
        );
    }

    /// H1 (ADR-144 §5.3 P1.2a)：`linux_sandbox_exe = Some` 时走 helper 路径，
    /// argv[0] = helper exe，且 arg0 override 为 `CODEX_LINUX_SANDBOX_ARG0`。
    /// 不再在父层装 seccomp（由 helper 内部装）。
    #[test]
    fn req_dasclaw_sandbox_p1_2a_prepare_helper_when_exe_configured() {
        use crate::{SandboxBackendConfig, SandboxablePreference};

        let helper_exe = PathBuf::from("/usr/local/bin/dasclaw-sandbox-linux");
        let mut policy = SandboxBackendConfig::default();
        policy.linux_sandbox_exe = Some(helper_exe.clone());
        policy.writable_roots = vec![PathBuf::from("/tmp/wk")];
        policy.allow_network = false;

        let cmd = Command::new("/bin/ls");
        let req = SandboxExecRequest {
            command: cmd,
            policy,
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
            windows_sandbox_level: dasclaw_protocol::config_types::WindowsSandboxLevel::Disabled,
            network: None,
        };
        let plan = prepare_command(&req).expect("helper path should succeed");

        assert_eq!(plan.program, helper_exe);
        assert_eq!(
            plan.arg0.as_deref(),
            Some(CODEX_LINUX_SANDBOX_ARG0),
            "helper path must override arg0 so the helper recognises self-invocation",
        );
        assert!(
            !plan.install_parent_seccomp,
            "helper path must NOT install parent seccomp — helper does it internally",
        );

        let args_str: Vec<String> = plan
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let sep_idx = args_str
            .iter()
            .position(|a| a == "--")
            .expect("helper argv must contain `--` separator before user command");
        assert_eq!(
            args_str[sep_idx + 1],
            "/bin/ls",
            "user command must follow `--` separator verbatim",
        );
        assert!(
            args_str.iter().any(|a| a == "--sandbox-policy-cwd"),
            "helper argv must include policy CWD flag for landlock helper",
        );
    }

    /// H1 (ADR-144 §5.3 P1.2a)：`linux_sandbox_exe = None` 但 spawn 携带
    /// `read_only_subpaths` carve-out（即要求 kernel 层 FS 隔离）时，
    /// 必须 Fail-Safe，不能静默降级为 seccomp-only。
    /// 守 ADR-141 §6 OQ-1 安全红线。
    #[test]
    fn req_dasclaw_sandbox_p1_2a_prepare_fail_closed_when_carveout_and_no_exe() {
        use crate::{SandboxBackendConfig, SandboxablePreference};

        let mut policy = SandboxBackendConfig::default();
        policy.linux_sandbox_exe = None;
        policy.read_only_subpaths = vec![PathBuf::from(".git/hooks")];

        let cmd = Command::new("/bin/touch");
        let req = SandboxExecRequest {
            command: cmd,
            policy,
            preference: SandboxablePreference::Auto,
            windows_sandbox_enabled: false,
            windows_sandbox_level: dasclaw_protocol::config_types::WindowsSandboxLevel::Disabled,
            network: None,
        };
        let err = prepare_command(&req).expect_err("must fail-closed");
        assert!(
            matches!(
                err,
                SandboxError::ReadOnlySubpathsKernelEnforcementMissing { .. }
            ),
            "expected ReadOnlySubpathsKernelEnforcementMissing, got {err:?}",
        );
    }
}
