//! Sandboxed shell executor — F4.6.7 Phase 2+3 port of the desktop
//! `OsExecutor` shim.
//!
//! Talks to [`dasclaw_exec::SandboxedExecutor`] using the 4-variant
//! [`dasclaw_workspace_cap::policy::SandboxPolicy`] directly. The old
//! 3-variant desktop `SandboxPolicy` lives at the desktop config-parse
//! layer only; the translation `legacy_policy_to_cap` stays there.
//!
//! Behaviour preserved verbatim from `desktop-client/ironclaw/src/sandbox/os_executor.rs`:
//! - per-call `SandboxedExecutor` construction so cwd/policy can vary
//! - session-scoped [`NetworkProxy`] cloned in on every execute
//! - Linux helper path resolved relative to the current executable
//! - 64KB output truncation with stderr concatenation
//! - tokio `spawn_blocking` + `tokio::time::timeout` envelope

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
use dasclaw_net_proxy::NetworkProxy;
use dasclaw_sandbox::SandboxablePreference;
use dasclaw_workspace_cap::policy::SandboxPolicy as CapPolicy;

/// 64KB 输出截断阈值（与桌面 Docker manager 路径一致）。
const MAX_OUTPUT_SIZE: usize = 64 * 1024;

/// Errors surfaced by [`SandboxedShellExecutor::execute`].
#[derive(Debug, thiserror::Error)]
pub enum ShellExecError {
    /// FullAccess 政策被请求但未通过双 opt-in。
    #[error("FullAccess policy requires opt-in (allow_full_access=true)")]
    FullAccessNotPermitted,
    /// 命令执行超过 timeout。
    #[error("command timed out after {0:?}")]
    Timeout(Duration),
    /// 下层 sandbox / process exec 失败。
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

/// 单次 sandbox 命令的输出。
#[derive(Debug, Clone)]
pub struct ExecOutput {
    /// 退出码。
    pub exit_code: i64,
    /// 标准输出（截断后）。
    pub stdout: String,
    /// 标准错误（截断后）。
    pub stderr: String,
    /// 合并输出（`stdout` + 可能的 `\n--- stderr ---\n` + `stderr`）。
    pub output: String,
    /// 命令实际耗时。
    pub duration: Duration,
    /// 是否触发了截断。
    pub truncated: bool,
}

/// Shell launch details reusable by callers that need to spawn the same
/// shell-wrapped command through a different process backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxedShellLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// OS 进程沙箱执行器。
///
/// 不持有 Docker / 代理状态，每次 [`execute`](Self::execute) 独立构造
/// [`SandboxedExecutor`]。这样 cwd / policy 可以按调用方期望逐次切换，
/// 不被首次 init 锁死。
///
/// 会话级 [`NetworkProxy`] 通过构造期一次性注入，[`execute`](Self::execute)
/// 时 `.clone()` 进 [`SandboxedExecutor`]——下游 `dasclaw_sandboxing` 据此
/// 在 Seatbelt profile 里 hole-punch HTTP/SOCKS 代理端口。`None` ⇒ 退化
/// 为沙箱内部完全禁网，fail-safe。
pub struct SandboxedShellExecutor {
    timeout: Duration,
    sandbox_pref: SandboxablePreference,
    allow_full_access: bool,
    network: Option<NetworkProxy>,
}

impl SandboxedShellExecutor {
    /// 创建执行器。
    ///
    /// - `timeout`: 单条命令最大执行时间。
    /// - `allow_full_access`: 必须为 `true` 才允许 `DangerFullAccess` 政策
    ///   落到 host 进程（双 opt-in）。
    /// - `network`: 可选会话级 [`NetworkProxy`]；`None` ⇒ 沙箱内部禁网。
    pub fn new(timeout: Duration, allow_full_access: bool, network: Option<NetworkProxy>) -> Self {
        Self {
            timeout,
            sandbox_pref: SandboxablePreference::Auto,
            allow_full_access,
            network,
        }
    }

    /// 在 OS 沙箱内执行 shell 命令。
    pub async fn execute(
        &self,
        command: &str,
        cwd: &Path,
        policy: CapPolicy,
        env: HashMap<String, String>,
    ) -> Result<ExecOutput, ShellExecError> {
        if matches!(policy, CapPolicy::DangerFullAccess) && !self.allow_full_access {
            tracing::error!(
                "DangerFullAccess execution requested but allow_full_access=false. Refusing."
            );
            return Err(ShellExecError::FullAccessNotPermitted);
        }

        let network = self.network.clone();
        // Linux helper exe 解析：相对当前主程序位置 `$EXE_DIR/dasclaw-sandbox-linux`。
        let linux_sandbox_exe = resolve_linux_sandbox_exe();
        let executor = SandboxedExecutor::new(policy, self.sandbox_pref, false, network)
            .with_linux_sandbox_exe(linux_sandbox_exe);

        let mut cmd = build_shell_command(command);
        cmd.envs(env);

        let req = ExecRequest {
            command: cmd,
            cwd: cwd.to_path_buf(),
        };
        let timeout = self.timeout;
        let start = Instant::now();

        let blocking = tokio::task::spawn_blocking(move || executor.execute(req));
        let output = match tokio::time::timeout(timeout, blocking).await {
            Ok(Ok(Ok(output))) => output,
            Ok(Ok(Err(exec_err))) => {
                return Err(ShellExecError::ExecutionFailed(exec_err.to_string()));
            }
            Ok(Err(join_err)) => {
                return Err(ShellExecError::ExecutionFailed(format!(
                    "blocking task panicked: {join_err}"
                )));
            }
            Err(_) => return Err(ShellExecError::Timeout(timeout)),
        };

        Ok(format_output(output, start.elapsed()))
    }
}

/// 把 shell 字符串包成可 spawn 的 [`Command`]。
fn build_shell_command(command: &str) -> Command {
    let (program, args) = shell_program_and_args(command);
    let mut c = Command::new(program);
    c.args(args);
    c
}

/// Build the shell launch contract for sandbox-aware streaming callers.
///
/// `DangerFullAccess` remains fail-closed here so callers cannot accidentally
/// bypass the buffered executor's double opt-in gate.
pub fn build_sandboxed_shell_launch_spec(
    command: &str,
    _cwd: &Path,
    policy: CapPolicy,
    env: HashMap<String, String>,
) -> Result<SandboxedShellLaunchSpec, ShellExecError> {
    if matches!(policy, CapPolicy::DangerFullAccess) {
        return Err(ShellExecError::FullAccessNotPermitted);
    }

    let (program, args) = shell_program_and_args(command);
    Ok(SandboxedShellLaunchSpec { program, args, env })
}

fn shell_program_and_args(command: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "windows") {
        (
            "cmd".to_string(),
            vec!["/C".to_string(), command.to_string()],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
}

/// 解析 Linux helper `dasclaw-sandbox-linux` 的绝对路径，相对当前可执行文件。
///
/// 仅在 Linux target 下返回 `Some`；macOS/Windows 返回 `None`。
fn resolve_linux_sandbox_exe() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        let helper = dir.join("dasclaw-sandbox-linux");
        if helper.exists() {
            Some(helper)
        } else {
            tracing::warn!(
                helper = %helper.display(),
                "dasclaw-sandbox-linux helper not found alongside main binary; \
                 spawns requiring kernel FS isolation will fail-closed"
            );
            None
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// 把 [`std::process::Output`] 适配回 [`ExecOutput`]，做截断 + stderr 合并。
fn format_output(raw: std::process::Output, duration: Duration) -> ExecOutput {
    let exit_code = raw.status.code().unwrap_or(-1) as i64;
    let mut stdout = String::from_utf8_lossy(&raw.stdout).into_owned();
    let mut stderr = String::from_utf8_lossy(&raw.stderr).into_owned();
    let half_max = MAX_OUTPUT_SIZE / 2;
    let mut truncated = false;

    if stdout.len() > half_max {
        let end = stdout.floor_char_boundary(half_max);
        stdout.truncate(end);
        truncated = true;
    }
    if stderr.len() > half_max {
        let end = stderr.floor_char_boundary(half_max);
        stderr.truncate(end);
        truncated = true;
    }

    let output = if stderr.is_empty() {
        stdout.clone()
    } else if stdout.is_empty() {
        stderr.clone()
    } else {
        format!("{}\n\n--- stderr ---\n{}", stdout, stderr)
    };

    ExecOutput {
        exit_code,
        stdout,
        stderr,
        output,
        duration,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_access_without_opt_in_refuses() {
        let executor = SandboxedShellExecutor::new(Duration::from_secs(5), false, None);
        let result = executor
            .execute(
                "echo hi",
                Path::new("/tmp"),
                CapPolicy::DangerFullAccess,
                HashMap::new(),
            )
            .await;
        assert!(matches!(
            result,
            Err(ShellExecError::FullAccessNotPermitted)
        ));
    }

    #[tokio::test]
    async fn timeout_returns_timeout_error() {
        let executor = SandboxedShellExecutor::new(Duration::from_millis(100), true, None);
        let result = executor
            .execute(
                "sleep 5",
                Path::new("/tmp"),
                CapPolicy::ReadOnly {
                    network_access: false,
                },
                HashMap::new(),
            )
            .await;
        // ReadOnly 在 macOS Seatbelt 下可能拦截 sh 自身，但 timeout 优先级更高。
        // 接受 Timeout 或 ExecutionFailed (sandbox error) 任一。
        match result {
            Err(ShellExecError::Timeout(_)) | Err(ShellExecError::ExecutionFailed(_)) => {}
            other => panic!("expected Timeout or ExecutionFailed, got {other:?}"),
        }
    }
}
