//! Hook trait seams exposed by `x_claw_agent`.
//!
//! These four traits define the crate boundary between the agent runtime
//! (owned by `x_claw_agent`) and the pluggable surrounding environment
//! (safety scanning, sandboxed execution, secret storage, approval flow).
//!
//! Design constraints:
//!
//! - **No ironclaw / claw-code dependency.** Traits use only `std`, `serde_json`,
//!   and simple owned types so this crate stays independently publishable and
//!   testable.
//! - **Async everywhere.** All hook methods are async so real implementations
//!   can do I/O (Docker exec, DB lookup, IPC to UI) without forcing the
//!   runtime to switch to blocking threads.
//! - **Per-trait error type.** Each hook surfaces its own error so the runtime
//!   can distinguish "safety blocked" from "sandbox failed" from "secret not
//!   found" and react appropriately.
//!
//! Default Noop/InMemory/AutoApprove implementations are provided so unit
//! tests of the runtime can construct a trivially-safe agent without wiring
//! real infrastructure.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// SafetyHook
// ---------------------------------------------------------------------------

/// Decision returned by [`SafetyHook`] methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyDecision {
    /// Allow the operation unchanged.
    Allow,
    /// Allow but note that the caller already rewrote the payload in-place
    /// (e.g. redacted a secret). The runtime should continue with the
    /// mutated value.
    Redact,
    /// Refuse the operation; `reason` is safe to surface to the user.
    Block { reason: String },
}

/// Errors raised by a [`SafetyHook`] implementation.
#[derive(Debug, thiserror::Error)]
pub enum SafetyError {
    #[error("safety hook internal error: {0}")]
    Internal(String),
}

/// Safety scanning applied at prompt / completion / tool-call boundaries.
///
/// Mutating `&mut String` / `&mut Value` in place lets the hook redact
/// secrets without forcing the runtime to clone the whole payload.
#[async_trait]
pub trait SafetyHook: Send + Sync {
    /// Called before a prompt is sent to the LLM. May mutate the prompt.
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError>;

    /// Called with a chunk of completion text before it is surfaced to the
    /// user or persisted. May mutate the completion.
    async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError>;

    /// Called before a tool call is executed. May mutate the arguments.
    async fn before_tool_call(
        &self,
        tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError>;

    /// Called with a tool output before it is returned to the LLM as a
    /// tool-result message. May mutate the output (e.g. redact secrets).
    async fn after_tool_output(
        &self,
        tool: &str,
        output: &mut String,
    ) -> Result<(), SafetyError>;
}

/// Default no-op implementation. Every operation is allowed unchanged.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSafetyHook;

#[async_trait]
impl SafetyHook for NoopSafetyHook {
    async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        Ok(SafetyDecision::Allow)
    }
    async fn after_completion(&self, _completion: &mut String) -> Result<(), SafetyError> {
        Ok(())
    }
    async fn before_tool_call(
        &self,
        _tool: &str,
        _args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
        Ok(SafetyDecision::Allow)
    }
    async fn after_tool_output(
        &self,
        _tool: &str,
        _output: &mut String,
    ) -> Result<(), SafetyError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SandboxExecutor
// ---------------------------------------------------------------------------

/// Request describing one command to run inside the sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecRequest {
    /// Shell command line.
    pub command: String,
    /// Working directory. Must be inside the sandbox-allowed mount set.
    pub cwd: PathBuf,
    /// Environment variables passed into the sandbox.
    pub env: HashMap<String, String>,
}

/// Result of a sandbox command execution.
///
/// Shape mirrors `ironclaw::sandbox::ExecOutput` (minus internal Duration so
/// the trait stays dependency-free).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecOutput {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    /// Combined stdout+stderr, suitable for surfacing to the LLM.
    pub output: String,
    /// Duration in milliseconds. `0` if the implementation does not measure.
    pub duration_ms: u64,
    pub truncated: bool,
}

/// Errors raised by a [`SandboxExecutor`].
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("sandbox not ready: {0}")]
    NotReady(String),
    #[error("sandbox execution failed: {0}")]
    ExecutionFailed(String),
    #[error("sandbox I/O error: {0}")]
    Io(String),
    #[error("sandbox policy violation: {0}")]
    PolicyViolation(String),
}

/// Runs model-requested code / file / network operations in an isolated
/// environment.
#[async_trait]
pub trait SandboxExecutor: Send + Sync {
    async fn run_bash(&self, req: SandboxExecRequest) -> Result<SandboxExecOutput, SandboxError>;

    /// Read a file through the sandbox. Implementations may reject reads
    /// outside the allowed mount set.
    async fn read_file(&self, path: &Path) -> Result<Vec<u8>, SandboxError>;

    /// Write a file through the sandbox.
    async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), SandboxError>;
}

/// Default implementation that refuses every operation. Use in unit tests
/// that don't exercise sandbox calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSandboxExecutor;

#[async_trait]
impl SandboxExecutor for NoopSandboxExecutor {
    async fn run_bash(&self, _req: SandboxExecRequest) -> Result<SandboxExecOutput, SandboxError> {
        Err(SandboxError::NotReady(
            "NoopSandboxExecutor does not execute commands".to_string(),
        ))
    }
    async fn read_file(&self, _path: &Path) -> Result<Vec<u8>, SandboxError> {
        Err(SandboxError::NotReady(
            "NoopSandboxExecutor does not perform file reads".to_string(),
        ))
    }
    async fn write_file(&self, _path: &Path, _data: &[u8]) -> Result<(), SandboxError> {
        Err(SandboxError::NotReady(
            "NoopSandboxExecutor does not perform file writes".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// SecretProvider
// ---------------------------------------------------------------------------

/// Opaque secret value. Wraps `String` so the runtime can pass it around
/// without the underlying value being accidentally logged via `Debug`.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Explicitly expose the secret value. Call sites using this should be
    /// few and audited.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        // Constant-time comparison is out of scope for a trait-layer container;
        // consumers that need it should compare `expose()` with a dedicated
        // crate such as `subtle`.
        self.0 == other.0
    }
}

impl Eq for SecretString {}

/// Errors raised by a [`SecretProvider`].
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("secret access denied: {0}")]
    AccessDenied(String),
    #[error("secret store I/O error: {0}")]
    Io(String),
}

/// Read-only view over the secret store sufficient for agent-side lookups.
///
/// Write operations (create/delete/rotate) intentionally live on a richer
/// store in the host application; the agent runtime should not need them.
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError>;
    async fn list_names(&self) -> Result<Vec<String>, SecretError>;
}

/// In-memory provider useful for tests.
#[derive(Debug, Default, Clone)]
pub struct InMemorySecrets {
    entries: HashMap<String, String>,
}

impl InMemorySecrets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.insert(key.into(), value.into());
    }
}

#[async_trait]
impl SecretProvider for InMemorySecrets {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        Ok(self.entries.get(key).map(|v| SecretString::new(v.clone())))
    }

    async fn list_names(&self) -> Result<Vec<String>, SecretError> {
        let mut names: Vec<String> = self.entries.keys().cloned().collect();
        names.sort();
        Ok(names)
    }
}

// ---------------------------------------------------------------------------
// ApprovalGate
// ---------------------------------------------------------------------------

/// Request presented to a user approval surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub tool: String,
    pub args: Value,
    /// Human-readable description of what the tool will do.
    pub description: String,
    /// Whether the approval surface should offer an "always approve this
    /// tool" option (`false` means every invocation must be confirmed,
    /// e.g. for destructive shell commands).
    pub allow_always: bool,
}

/// Outcome returned from the approval surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalOutcome {
    /// Run the tool with the original arguments.
    Approved,
    /// Run the tool, and remember the approval for future invocations of
    /// the same tool within this session.
    ApprovedAlways,
    /// Run the tool with replaced arguments.
    ApprovedWithArgs(Value),
    /// Do not run the tool. `reason` is safe to surface to the user.
    Denied { reason: String },
}

/// Errors raised by an [`ApprovalGate`].
#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval request timed out")]
    Timeout,
    #[error("approval surface disconnected: {0}")]
    Disconnected(String),
    #[error("approval surface internal error: {0}")]
    Internal(String),
}

/// Mediator between the agent loop and whatever surface asks the user to
/// approve a tool invocation (Tauri dialog, web SSE, CLI TTY prompt, ...).
#[async_trait]
pub trait ApprovalGate: Send + Sync {
    async fn request(&self, req: ApprovalRequest) -> Result<ApprovalOutcome, ApprovalError>;
}

/// Auto-approve every tool invocation. Useful in headless tests and in
/// `auto_approve_tools = true` deployments.
#[derive(Debug, Default, Clone, Copy)]
pub struct AutoApproveGate;

#[async_trait]
impl ApprovalGate for AutoApproveGate {
    async fn request(&self, _req: ApprovalRequest) -> Result<ApprovalOutcome, ApprovalError> {
        Ok(ApprovalOutcome::Approved)
    }
}

/// Reject every tool invocation. Useful for regression tests that want to
/// prove a code path never reaches the tool executor.
#[derive(Debug, Default, Clone, Copy)]
pub struct DenyAllGate;

#[async_trait]
impl ApprovalGate for DenyAllGate {
    async fn request(&self, _req: ApprovalRequest) -> Result<ApprovalOutcome, ApprovalError> {
        Ok(ApprovalOutcome::Denied {
            reason: "DenyAllGate rejects every tool call".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn noop_safety_hook_allows_all() {
        let hook = NoopSafetyHook;
        let mut prompt = "hi".to_string();
        assert_eq!(
            hook.before_prompt(&mut prompt).await.unwrap(),
            SafetyDecision::Allow
        );
        assert_eq!(prompt, "hi");

        let mut completion = "reply".to_string();
        hook.after_completion(&mut completion).await.unwrap();

        let mut args = json!({"x": 1});
        assert_eq!(
            hook.before_tool_call("echo", &mut args).await.unwrap(),
            SafetyDecision::Allow
        );

        let mut output = "ok".to_string();
        hook.after_tool_output("echo", &mut output).await.unwrap();
    }

    #[tokio::test]
    async fn noop_sandbox_executor_refuses() {
        let sb = NoopSandboxExecutor;
        let req = SandboxExecRequest {
            command: "ls".to_string(),
            cwd: PathBuf::from("/"),
            env: HashMap::new(),
        };
        assert!(matches!(
            sb.run_bash(req).await,
            Err(SandboxError::NotReady(_))
        ));
        assert!(matches!(
            sb.read_file(Path::new("/etc/passwd")).await,
            Err(SandboxError::NotReady(_))
        ));
        assert!(matches!(
            sb.write_file(Path::new("/tmp/x"), b"data").await,
            Err(SandboxError::NotReady(_))
        ));
    }

    #[tokio::test]
    async fn in_memory_secrets_round_trip() {
        let mut store = InMemorySecrets::new();
        store.insert("api_key", "sk-123");
        store.insert("token", "tok-abc");

        let got = store.get("api_key").await.unwrap().unwrap();
        assert_eq!(got.expose(), "sk-123");
        assert!(store.get("missing").await.unwrap().is_none());

        let mut names = store.list_names().await.unwrap();
        names.sort();
        assert_eq!(names, vec!["api_key".to_string(), "token".to_string()]);
    }

    #[test]
    fn secret_string_debug_does_not_leak() {
        let s = SecretString::new("super-secret-value");
        let rendered = format!("{:?}", s);
        assert!(!rendered.contains("super-secret-value"));
        assert!(rendered.contains("REDACTED"));
    }

    #[tokio::test]
    async fn auto_approve_gate_approves() {
        let gate = AutoApproveGate;
        let out = gate
            .request(ApprovalRequest {
                tool: "shell".to_string(),
                args: json!({"cmd": "rm -rf /"}),
                description: "dangerous".to_string(),
                allow_always: false,
            })
            .await
            .unwrap();
        assert_eq!(out, ApprovalOutcome::Approved);
    }

    #[tokio::test]
    async fn deny_all_gate_denies() {
        let gate = DenyAllGate;
        let out = gate
            .request(ApprovalRequest {
                tool: "shell".to_string(),
                args: json!({}),
                description: "".to_string(),
                allow_always: true,
            })
            .await
            .unwrap();
        match out {
            ApprovalOutcome::Denied { reason } => assert!(!reason.is_empty()),
            other => panic!("expected Denied, got {:?}", other),
        }
    }
}
