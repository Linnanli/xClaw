//! Shared sandbox-backed `bash` `ToolExecutor` for W6.6 A6 e2e family
//! (ADR-153 §1.1 A6 / A6b / A6c).
//!
//! Wires [`dasclaw_exec::SandboxedExecutor`] under a caller-supplied
//! [`dasclaw_workspace_cap::policy::SandboxPolicy`] so each test can pin
//! a different policy (ReadOnly / WorkspaceWrite / carve-out probe)
//! against the same agent-loop scaffold.
//!
//! The sibling test [`safety_sandbox_default_deny_e2e.rs`] inlines an
//! equivalent helper for historical reasons; new tests under this family
//! must include this module via
//! `#[path = "fixtures/sandbox_bash_executor.rs"] mod sandbox_bash_executor;`
//! rather than copy-paste the executor.
#![allow(dead_code)] // Each test binary uses only a subset.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{ToolCall, ToolDefinition, ToolResult};
use dasclaw_core::response_types::RespondOutput;
use dasclaw_core::traits::HostError;
use dasclaw_exec::{ExecRequest, ProcessExecutor, SandboxedExecutor};
use dasclaw_runtime::ToolExecutor;
use dasclaw_sandbox::SandboxablePreference;
use dasclaw_workspace_cap::policy::SandboxPolicy;
use serde_json::json;

#[path = "agent_loop_fixtures.rs"]
mod agent_loop_fixtures;
pub use agent_loop_fixtures::{ScriptedResponder, text_turn, tool_call_turn};

/// `ToolExecutor` that pipes the `command` argument of each call into
/// `bash -c` through the real OS sandbox. Errors from the sandbox
/// (`ExecError::Sandbox`) and non-zero exits both surface as
/// `is_error=true` so the agent loop sees them as tool failures, matching
/// the A6/A6b/A6c matrix expectation.
pub struct SandboxedBashExecutor {
    inner: Arc<SandboxedExecutor>,
    cwd: PathBuf,
}

impl SandboxedBashExecutor {
    pub fn new(policy: SandboxPolicy, cwd: PathBuf) -> Self {
        Self {
            inner: Arc::new(SandboxedExecutor::new(
                policy,
                SandboxablePreference::Require,
                false,
                None,
            )),
            cwd,
        }
    }
}

#[async_trait]
impl ToolExecutor for SandboxedBashExecutor {
    async fn execute(&self, call: &ToolCall) -> Result<ToolResult, HostError> {
        let command = call
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::from("missing `command` arg"))?
            .to_string();

        let mut cmd = Command::new("/bin/bash");
        cmd.arg("-c").arg(&command);

        let inner = Arc::clone(&self.inner);
        let cwd = self.cwd.clone();
        // SandboxedExecutor::execute is sync + spawns sandbox-exec, so
        // hop to a blocking pool to keep the tokio runtime responsive.
        let outcome =
            tokio::task::spawn_blocking(move || inner.execute(ExecRequest { command: cmd, cwd }))
                .await
                .map_err(|e| HostError::from(format!("blocking pool: {e}")))?;

        let (is_error, content) = match outcome {
            Ok(output) if output.status.success() => {
                (false, String::from_utf8_lossy(&output.stdout).into_owned())
            }
            Ok(output) => {
                // Sandbox let the command run but the command itself
                // failed (e.g. shell error on a denied write). For A6/A6b/A6c
                // this still counts as a sandbox-enforced denial when
                // stderr names sandbox-exec / Operation not permitted.
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                (
                    true,
                    format!("sandbox: command exited with {}: {stderr}", output.status),
                )
            }
            Err(err) => (true, format!("sandbox: denied: {err}")),
        };

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            name: call.name.clone(),
            content,
            is_error,
        })
    }
}

/// Standard `bash` tool definition used by all A6 family e2e tests.
pub fn bash_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "bash".to_string(),
        description: "Execute a bash command inside the OS sandbox.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" }
            },
            "required": ["command"]
        }),
    }
}

/// Acknowledgement turn the scripted model emits after seeing a tool
/// error. Kept short so tests focus on whether the loop actually halted
/// on the sandbox decision rather than on prompt content.
pub fn ack_turn() -> RespondOutput {
    text_turn("understood: sandbox denied the write")
}
