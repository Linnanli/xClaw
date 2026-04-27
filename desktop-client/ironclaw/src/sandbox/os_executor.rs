//! W3.1a: OS-level sandbox executor backed by [`dasclaw_exec`].
//!
//! 现状：旧 [`super::manager::SandboxManager`] 走 Docker + 网络代理路线。
//! W3 路线决定改走 codex 兼容的进程级 sandbox（macOS Seatbelt /
//! Linux Landlock+seccomp，由 [`dasclaw_sandbox`] 提供）。
//!
//! 本 PR (W3.1a) 只**新增** `OsExecutor`，不动 Docker manager — 让 shell.rs
//! 后续可以按 backend 选择切换。完整删 Docker 在 W3.1c。
//!
//! ## 政策映射
//!
//! 旧 [`super::config::SandboxPolicy`] 只有 3 变体（ReadOnly /
//! WorkspaceWrite / FullAccess），对应 [`ironclaw_workspace_cap::policy::SandboxPolicy`]
//! 的 4 变体精确表达：
//!
//! | 旧 | 新（workspace_cap） | 备注 |
//! |----|-------------------|------|
//! | `ReadOnly` | `ReadOnly { network_access: false }` | 全盘只读 + 禁网 |
//! | `WorkspaceWrite` | `WorkspaceWrite { writable_roots: [], network_access: false, .. }` | cwd 由 dasclaw_exec 隐式加入 |
//! | `FullAccess` | `DangerFullAccess` | 双 opt-in 守卫保留 |
//!
//! ## 与旧 SandboxManager 的契约对齐
//!
//! - 输入：`command: &str`（shell 字符串，仍用 `sh -c` / `cmd /C` 包裹）
//! - 输出：[`super::manager::ExecOutput`]（不变，避免上层重写）
//! - 错误：[`super::error::SandboxError`]（不变）
//! - 异步：tokio runtime 上 spawn_blocking + timeout

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
use dasclaw_sandbox::SandboxablePreference;
use ironclaw_workspace_cap::policy::SandboxPolicy as CapPolicy;

use super::config::SandboxPolicy;
use super::error::{Result, SandboxError};
use super::manager::ExecOutput;

/// 64KB 输出截断阈值（与 Docker manager 路径一致）。
const MAX_OUTPUT_SIZE: usize = 64 * 1024;

/// 把 ironclaw 旧 3-变体 SandboxPolicy 投影到 workspace_cap 4-变体精确表达。
fn legacy_policy_to_cap(policy: SandboxPolicy) -> CapPolicy {
    match policy {
        SandboxPolicy::ReadOnly => CapPolicy::ReadOnly {
            network_access: false,
        },
        SandboxPolicy::WorkspaceWrite => CapPolicy::new_workspace_write_policy(),
        SandboxPolicy::FullAccess => CapPolicy::DangerFullAccess,
    }
}

/// OS 进程沙箱执行器。
///
/// 不持有 Docker / 代理状态，每次 `execute` 独立构造 [`SandboxedExecutor`]。
/// 这样 cwd / policy 可以按调用方期望逐次切换，不被首次 init 锁死。
pub struct OsExecutor {
    timeout: Duration,
    sandbox_pref: SandboxablePreference,
    allow_full_access: bool,
}

impl OsExecutor {
    /// 创建执行器。
    ///
    /// - `timeout`: 单条命令最大执行时间。
    /// - `allow_full_access`: 必须为 `true` 才允许 `FullAccess` 政策落到 host
    ///   进程（双 opt-in，与 `SANDBOX_ALLOW_FULL_ACCESS` 等价）。
    pub fn new(timeout: Duration, allow_full_access: bool) -> Self {
        Self {
            timeout,
            sandbox_pref: SandboxablePreference::Auto,
            allow_full_access,
        }
    }

    /// 在 OS 沙箱内执行 shell 命令。
    ///
    /// 旧 `SandboxManager::execute_with_policy` 的等价接口。
    pub async fn execute(
        &self,
        command: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        env: HashMap<String, String>,
    ) -> Result<ExecOutput> {
        if policy == SandboxPolicy::FullAccess && !self.allow_full_access {
            tracing::error!(
                "FullAccess execution requested but allow_full_access=false. Refusing."
            );
            return Err(SandboxError::Config {
                reason: "FullAccess policy requires SANDBOX_ALLOW_FULL_ACCESS=true".to_string(),
            });
        }

        let cap_policy = legacy_policy_to_cap(policy);
        let executor = SandboxedExecutor::new(cap_policy, self.sandbox_pref, false);

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
                return Err(SandboxError::ExecutionFailed {
                    reason: exec_err.to_string(),
                });
            }
            Ok(Err(join_err)) => {
                return Err(SandboxError::ExecutionFailed {
                    reason: format!("blocking task panicked: {join_err}"),
                });
            }
            Err(_) => return Err(SandboxError::Timeout(timeout)),
        };

        Ok(format_output(output, start.elapsed()))
    }
}

/// 把 shell 字符串包成可 spawn 的 [`Command`]（与旧 manager `execute_direct` 同行为）。
fn build_shell_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
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
        let end = crate::util::floor_char_boundary(&stdout, half_max);
        stdout.truncate(end);
        truncated = true;
    }
    if stderr.len() > half_max {
        let end = crate::util::floor_char_boundary(&stderr, half_max);
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

    #[test]
    fn legacy_readonly_maps_to_cap_readonly_no_network() {
        let cap = legacy_policy_to_cap(SandboxPolicy::ReadOnly);
        assert!(matches!(
            cap,
            CapPolicy::ReadOnly {
                network_access: false
            }
        ));
    }

    #[test]
    fn legacy_workspace_write_maps_to_cap_workspace_write() {
        let cap = legacy_policy_to_cap(SandboxPolicy::WorkspaceWrite);
        assert!(matches!(cap, CapPolicy::WorkspaceWrite { .. }));
    }

    #[test]
    fn legacy_full_access_maps_to_danger_full() {
        let cap = legacy_policy_to_cap(SandboxPolicy::FullAccess);
        assert!(matches!(cap, CapPolicy::DangerFullAccess));
    }

    #[tokio::test]
    async fn full_access_without_opt_in_refuses() {
        let executor = OsExecutor::new(Duration::from_secs(5), false);
        let result = executor
            .execute(
                "echo hi",
                Path::new("/tmp"),
                SandboxPolicy::FullAccess,
                HashMap::new(),
            )
            .await;
        assert!(matches!(result, Err(SandboxError::Config { .. })));
    }

    #[tokio::test]
    async fn timeout_returns_timeout_error() {
        let executor = OsExecutor::new(Duration::from_millis(100), true);
        // sleep 远超 100ms,在 macOS / Linux 上都可用
        let result = executor
            .execute(
                "sleep 5",
                Path::new("/tmp"),
                SandboxPolicy::ReadOnly,
                HashMap::new(),
            )
            .await;
        // ReadOnly 在 macOS Seatbelt 下可能拦截 sh 自身,但 timeout 优先级更高
        // 接受 Timeout 或 ExecutionFailed (sandbox error) 任一
        match result {
            Err(SandboxError::Timeout(_)) | Err(SandboxError::ExecutionFailed { .. }) => {}
            other => panic!("expected Timeout or ExecutionFailed, got {other:?}"),
        }
    }
}
