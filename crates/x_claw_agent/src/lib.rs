//! `x_claw_agent` — agent runtime fork of `claw-code` for the x-claw project.
//!
//! This crate starts as an **empty scaffold** (Phase 3 Step B). Subsequent steps
//! will:
//!
//! - Step C: define Hook traits (`SafetyHook`, `SandboxExecutor`, `SecretProvider`,
//!   `ApprovalGate`) in [`hooks`]. **Done.**
//! - Step D: port the runtime loop from ironclaw's `agent/` tree into
//!   [`runtime`] with hook insertion points. **Pending.**
//!
//! See [`../UPSTREAM_BASELINE.md`](../UPSTREAM_BASELINE.md) for the chosen
//! upstream commit and porting log.

pub mod agentic_loop;
pub mod compaction;
pub mod context_monitor;
pub mod hooks;
pub mod intent;
pub mod messages;
pub mod reasoning_ctx;
pub mod response_types;
pub mod session;
pub mod submission;
pub mod task;
pub mod traits;
pub mod undo;

pub use hooks::{
    ApprovalError, ApprovalGate, ApprovalOutcome, ApprovalRequest, AutoApproveGate, DenyAllGate,
    InMemorySecrets, NoopSafetyHook, NoopSandboxExecutor, SafetyDecision, SafetyError, SafetyHook,
    SandboxError, SandboxExecOutput, SandboxExecRequest, SandboxExecutor, SecretError,
    SecretProvider, SecretString,
};
pub use messages::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, ImageUrl,
    ModelMetadata, Role, ToolCall, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
    ToolResult, UnsupportedParam, generate_tool_call_id, sanitize_tool_messages,
    strip_unsupported_completion_params, strip_unsupported_tool_params,
};
pub use traits::{HostError, LlmCompleter, WorkspaceWriter};

/// Crate version string, exposed so downstream crates can surface the baseline
/// to operators without reparsing `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_has_version() {
        assert!(!VERSION.is_empty());
    }
}
