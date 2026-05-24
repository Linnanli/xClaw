//! Shared git command runner.
//!
//! Executes `git` sub-commands with timeout, output capture, and error mapping.
//! All git tools delegate to this instead of duplicating process management.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use dasclaw_tool::ToolError;

/// Default timeout for git commands (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum output size before truncation (256 KB).
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Result of a git command execution.
pub(super) struct GitOutput {
    pub stdout: String,
    pub exit_code: i32,
}

/// Run a git sub-command in a given working directory.
///
/// # Arguments
/// * `args` — Arguments after `git` (e.g. `["status", "--porcelain=v1"]`)
/// * `workdir` — Working directory (must be inside a git repo)
/// * `timeout` — Optional custom timeout; defaults to 30s
pub(super) async fn run_git(
    args: &[&str],
    workdir: &Path,
    timeout: Option<Duration>,
) -> Result<GitOutput, ToolError> {
    let timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);

    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat");

    let child = cmd
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn git: {}", e)))?;

    let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

    match result {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();

            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.is_empty() {
                    stdout.push_str("\n--- stderr ---\n");
                }
                stdout.push_str(&stderr);
            }

            Ok(GitOutput {
                stdout: truncate(&stdout),
                exit_code,
            })
        }
        Ok(Err(e)) => Err(ToolError::ExecutionFailed(format!("git I/O error: {}", e))),
        Err(_) => {
            // child is dropped here, kill_on_drop(true) ensures cleanup
            Err(ToolError::Timeout(timeout))
        }
    }
}

/// Resolve the working directory for git operations.
///
/// If `path` is provided, validate it; otherwise use `base_dir` or CWD.
pub(super) fn resolve_workdir(
    path: Option<&str>,
    base_dir: Option<&Path>,
) -> Result<PathBuf, ToolError> {
    if let Some(p) = path {
        let dir = PathBuf::from(p);
        if !dir.exists() {
            return Err(ToolError::InvalidParameters(format!(
                "Path does not exist: {}",
                p
            )));
        }
        return Ok(dir);
    }
    if let Some(base) = base_dir {
        return Ok(base.to_path_buf());
    }
    std::env::current_dir()
        .map_err(|e| ToolError::ExecutionFailed(format!("Cannot determine CWD: {}", e)))
}

fn truncate(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let cut = &s[..MAX_OUTPUT_BYTES];
    format!(
        "{}\n\n--- output truncated ({} bytes total) ---",
        cut,
        s.len()
    )
}
