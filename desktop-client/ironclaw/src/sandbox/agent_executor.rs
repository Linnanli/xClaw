//! `x_claw_agent::SandboxExecutor` adapter for [`SandboxManager`].
//!
//! > **⚠️ Phase 3 未接线 — 见 ADR-001**
//! >
//! > 本 adapter 在 D-3 阶段按"进程内 sandbox hook"假设建立，后经 E 阶段
//! > 深入排查确认：上游 ironclaw 的真实沙箱架构是**进程外 daemon**
//! > (`src/bridge/sandbox/` + `src/bin/sandbox_daemon.rs`)，而非在 agent
//! > 主进程里 hook 一个 executor。本仓的 `SandboxManager`（engine v1）
//! > 在 agent 主进程里 `SandboxManager::new` 调用次数 = 0，上游 main 一致。
//! >
//! > 决策（ADR-001）：
//! > - Phase 3 **不把本 adapter 接入 `HookBundle.sandbox`**
//! > - `hook_bundle_with_safety()` helper 里 `sandbox` 槽保持 `NoopSandboxExecutor`
//! > - 真实沙箱能力对齐推到 Phase 4 E-2~E-7（port 上游 engine v2 + bridge + daemon）
//! > - 本 adapter 保留不删，作为"若未来需要把 engine v1 `SandboxManager`
//! >   挂进进程内 hook 的参考实现" + 错误假设的历史记录
//! >
//! > 相关文档：`docs/plans/architecture-refactor/adr-001-sandbox-hook-not-wired-in-phase3.md`
//!
//! This adapter lets the agent runtime (in `x_claw_agent`) drive bash
//! execution and file I/O through ironclaw's Docker sandbox without taking
//! a direct dependency on ironclaw internals.
//!
//! # File I/O semantics
//!
//! The `SandboxExecutor` trait exposes `read_file` / `write_file`, but the
//! underlying `SandboxManager::execute` is a process-level API (no direct
//! file API). For the `WorkspaceWrite` / `ReadOnly` policies the agent's
//! workspace is mounted at `/workspace`, so we delegate file I/O to the
//! host filesystem **only when the path is inside the configured allowed
//! workspace root**. Paths outside raise `SandboxError::PolicyViolation`.
//! This is a deliberately conservative default; a container-side file API
//! can be swapped in later without changing the trait.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use x_claw_agent::{
    SandboxError as AgentSandboxError, SandboxExecOutput, SandboxExecRequest, SandboxExecutor,
};

use crate::sandbox::error::SandboxError as IronclawSandboxError;
use crate::sandbox::manager::{ExecOutput, SandboxManager};

/// Adapter exposing [`SandboxManager`] as an `x_claw_agent::SandboxExecutor`.
///
/// Wraps the manager in `Arc` so the same sandbox can be shared across the
/// agent runtime, routines engine, and container-backed workers.
///
/// `workspace_root` bounds the file I/O surface: paths that do not live
/// inside this directory are rejected with `PolicyViolation`. This mirrors
/// the `WorkspaceWrite` sandbox policy guarantee.
#[derive(Clone)]
pub struct SandboxAgentExecutor {
    manager: Arc<SandboxManager>,
    workspace_root: PathBuf,
}

impl SandboxAgentExecutor {
    pub fn new(manager: Arc<SandboxManager>, workspace_root: PathBuf) -> Self {
        Self {
            manager,
            workspace_root,
        }
    }

    pub fn manager(&self) -> &SandboxManager {
        &self.manager
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Reject paths that escape the configured workspace root.
    ///
    /// `canonicalize` is deliberately NOT used here so the check still
    /// works for files that do not yet exist (e.g. `write_file` targets).
    /// Symlink traversal is handled at the filesystem layer by the sandbox
    /// mount configuration.
    fn check_within_workspace(&self, path: &Path) -> Result<PathBuf, AgentSandboxError> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        };

        // Normalize `..` components without touching the filesystem.
        let normalized = normalize_path(&absolute);

        if !normalized.starts_with(&self.workspace_root) {
            return Err(AgentSandboxError::PolicyViolation(format!(
                "path {} is outside the configured workspace root {}",
                normalized.display(),
                self.workspace_root.display()
            )));
        }

        Ok(normalized)
    }
}

#[async_trait]
impl SandboxExecutor for SandboxAgentExecutor {
    async fn run_bash(
        &self,
        req: SandboxExecRequest,
    ) -> Result<SandboxExecOutput, AgentSandboxError> {
        // Bound cwd to the workspace so callers cannot trick us into
        // changing into a host directory.
        let cwd = self.check_within_workspace(&req.cwd)?;

        let env: HashMap<String, String> = req.env;
        let result = self.manager.execute(&req.command, &cwd, env).await;

        result.map(exec_output_to_agent).map_err(to_agent_error)
    }

    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, AgentSandboxError> {
        let safe = self.check_within_workspace(path)?;
        tokio::fs::read(&safe)
            .await
            .map_err(|e| AgentSandboxError::Io(format!("read {}: {e}", safe.display())))
    }

    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), AgentSandboxError> {
        let safe = self.check_within_workspace(path)?;
        if let Some(parent) = safe.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AgentSandboxError::Io(format!("mkdir {}: {e}", parent.display())))?;
        }
        tokio::fs::write(&safe, data)
            .await
            .map_err(|e| AgentSandboxError::Io(format!("write {}: {e}", safe.display())))
    }
}

fn exec_output_to_agent(out: ExecOutput) -> SandboxExecOutput {
    SandboxExecOutput {
        exit_code: out.exit_code,
        stdout: out.stdout,
        stderr: out.stderr,
        output: out.output,
        duration_ms: out.duration.as_millis() as u64,
        truncated: out.truncated,
    }
}

fn to_agent_error(err: IronclawSandboxError) -> AgentSandboxError {
    match err {
        IronclawSandboxError::DockerNotAvailable { reason } => AgentSandboxError::NotReady(reason),
        IronclawSandboxError::ContainerCreationFailed { reason }
        | IronclawSandboxError::ContainerStartFailed { reason } => {
            AgentSandboxError::NotReady(reason)
        }
        IronclawSandboxError::ExecutionFailed { reason } => {
            AgentSandboxError::ExecutionFailed(reason)
        }
        IronclawSandboxError::Timeout(d) => {
            AgentSandboxError::ExecutionFailed(format!("timeout after {d:?}"))
        }
        IronclawSandboxError::ResourceLimitExceeded { resource, limit } => {
            AgentSandboxError::PolicyViolation(format!("{resource} limit {limit} exceeded"))
        }
        IronclawSandboxError::ProxyError { reason } => AgentSandboxError::Io(reason),
        IronclawSandboxError::NetworkBlocked { reason } => {
            AgentSandboxError::PolicyViolation(reason)
        }
        IronclawSandboxError::CredentialInjectionFailed { domain, reason } => {
            AgentSandboxError::ExecutionFailed(format!(
                "credential injection for {domain}: {reason}"
            ))
        }
        IronclawSandboxError::Docker(e) => AgentSandboxError::ExecutionFailed(e.to_string()),
        IronclawSandboxError::Io(e) => AgentSandboxError::Io(e.to_string()),
        IronclawSandboxError::Config { reason } => AgentSandboxError::NotReady(reason),
    }
}

/// Resolve `.` / `..` components without touching the filesystem.
///
/// We use a manual normalizer (not `canonicalize`) so paths that do not
/// yet exist still get checked correctly.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::config::SandboxConfig;

    fn executor(workspace: PathBuf) -> SandboxAgentExecutor {
        // We don't initialize the manager (no Docker in unit tests); we only
        // exercise the path-check + file-IO layer here.
        let cfg = SandboxConfig::default();
        let mgr = Arc::new(SandboxManager::new(cfg));
        SandboxAgentExecutor::new(mgr, workspace)
    }

    #[tokio::test]
    async fn check_within_workspace_allows_relative_path_inside_root() {
        let tmp = tempfile::tempdir().unwrap();
        let exec = executor(tmp.path().to_path_buf());
        let resolved = exec
            .check_within_workspace(Path::new("sub/file.txt"))
            .unwrap();
        assert!(resolved.starts_with(tmp.path()));
    }

    #[tokio::test]
    async fn check_within_workspace_rejects_parent_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let exec = executor(tmp.path().to_path_buf());
        // `../../../etc/passwd` resolves to something outside the root.
        let err = exec
            .check_within_workspace(Path::new("../../../etc/passwd"))
            .unwrap_err();
        assert!(
            matches!(err, AgentSandboxError::PolicyViolation(_)),
            "expected PolicyViolation, got {err:?}"
        );
    }

    #[tokio::test]
    async fn check_within_workspace_rejects_absolute_foreign_path() {
        let tmp = tempfile::tempdir().unwrap();
        let exec = executor(tmp.path().to_path_buf());
        let err = exec
            .check_within_workspace(Path::new("/etc/passwd"))
            .unwrap_err();
        assert!(matches!(err, AgentSandboxError::PolicyViolation(_)));
    }

    #[tokio::test]
    async fn write_then_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let exec = executor(tmp.path().to_path_buf());
        let rel = Path::new("nested/hello.txt");
        exec.write_file(rel, b"hi").await.unwrap();
        let got = exec.read_file(rel).await.unwrap();
        assert_eq!(got, b"hi");
    }

    #[tokio::test]
    async fn write_outside_workspace_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let exec = executor(tmp.path().to_path_buf());
        let err = exec
            .write_file(Path::new("/tmp/should-not-work.txt"), b"x")
            .await
            .unwrap_err();
        assert!(matches!(err, AgentSandboxError::PolicyViolation(_)));
    }

    #[test]
    fn normalize_handles_dotdot() {
        let p = normalize_path(Path::new("/a/b/../c"));
        assert_eq!(p, PathBuf::from("/a/c"));
    }

    #[test]
    fn normalize_handles_trailing_dot() {
        let p = normalize_path(Path::new("/a/./b"));
        assert_eq!(p, PathBuf::from("/a/b"));
    }
}
