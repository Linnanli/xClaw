//! `BashValidationHook` — wraps [`x_claw_agent::bash_validation::validate_command`]
//! as a [`SafetyHook`] implementation.
//!
//! # Status
//!
//! **Scaffold only.** This module is intentionally **not registered in any
//! production `SafetyHook` chain**. It exists so a future PR (tracked by
//! parent issue [#73]) can wire it into ironclaw's `IronclawSafetyHook`
//! composition after the red-line decisions are settled (PermissionMode
//! injection path, Warn handling policy, error-message redaction contract).
//!
//! See [`docs/plans/architecture-refactor/32-execution-plan.md`] §W4 D10.
//!
//! # Design (Fail-Safe)
//!
//! Only `before_tool_call` does real work. The other three [`SafetyHook`]
//! methods pass through unchanged so this hook composes cleanly with other
//! safety hooks (e.g. `IronclawSafetyHook`, prompt-injection scanners).
//!
//! For tools whose name matches [`BashValidationHook::bash_tool_names`]
//! (default `["bash", "shell", "BashTool"]`), the hook:
//!
//! 1. Extracts the `command` field from `args`. Missing or non-string ⇒
//!    [`SafetyDecision::Block`] (Fail-Safe: never let a malformed bash call
//!    through).
//! 2. Calls `validate_command(cmd, mode, workspace)`.
//! 3. Maps the result:
//!    - `Allow` ⇒ [`SafetyDecision::Allow`].
//!    - `Block { reason }` ⇒ [`SafetyDecision::Block { reason }`]. The
//!      `reason` string comes from `validate_command`, which by design
//!      references the *command name or pattern*, **not the full original
//!      argument string** — see the safety-audit test below.
//!    - `Warn { message }` ⇒ **TODO** ([#73]): emit `tracing::warn!` and
//!      return `Allow`. The final policy (approval-gate / silent allow /
//!      escalate) is an ADR-redline decision and is not made here.
//!
//! For non-bash tools the hook short-circuits to `Allow` so it can be
//! installed globally without disturbing other tool calls.
//!
//! # Why this is not yet wired up
//!
//! Three red-line decisions still belong to parent issue [#73]:
//!
//! - How `PermissionMode` is resolved per invocation (session config /
//!   per-request / workspace-level).
//! - How `Warn` results are surfaced to the user (approval gate vs log only).
//! - How [`SafetyError`] returned by hooks is finalised into a `Block` at
//!   the orchestrator boundary (the agent loop must Fail-Safe, never
//!   Fail-Open, but the exact orchestration is the parent issue's scope).
//!
//! [#73]: https://github.com/Linnanli/xClaw/issues/73

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use x_claw_agent::bash_validation::{ValidationResult, validate_command};
use x_claw_agent::permissions::PermissionMode;
use x_claw_agent::{SafetyDecision, SafetyError, SafetyHook};

/// Default list of tool names treated as bash invocations.
///
/// Tool names are matched case-sensitively against [`SafetyHook::before_tool_call`]'s
/// `tool` argument. Callers can override via [`BashValidationHook::with_tool_names`]
/// when their tool registry uses different names.
pub const DEFAULT_BASH_TOOL_NAMES: &[&str] = &["bash", "shell", "BashTool"];

/// Hook that runs `validate_command` on bash tool calls.
///
/// See module-level documentation for the full contract.
#[derive(Debug, Clone)]
pub struct BashValidationHook {
    permission_mode: PermissionMode,
    workspace: PathBuf,
    bash_tool_names: Vec<String>,
}

impl BashValidationHook {
    /// Create a hook with the given permission mode and workspace root.
    ///
    /// Uses [`DEFAULT_BASH_TOOL_NAMES`] for the tool-name allowlist.
    #[must_use]
    pub fn new(permission_mode: PermissionMode, workspace: PathBuf) -> Self {
        Self {
            permission_mode,
            workspace,
            bash_tool_names: DEFAULT_BASH_TOOL_NAMES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }

    /// Override the bash tool-name allowlist.
    ///
    /// Useful when the tool registry exposes bash under a custom name.
    #[must_use]
    pub fn with_tool_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bash_tool_names = names.into_iter().map(Into::into).collect();
        self
    }

    /// Returns `true` if `tool` should be treated as a bash invocation.
    fn is_bash_tool(&self, tool: &str) -> bool {
        self.bash_tool_names.iter().any(|n| n == tool)
    }
}

#[async_trait]
impl SafetyHook for BashValidationHook {
    async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        Ok(SafetyDecision::Allow)
    }

    async fn after_completion(&self, _completion: &mut String) -> Result<(), SafetyError> {
        Ok(())
    }

    async fn before_tool_call(
        &self,
        tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
        if !self.is_bash_tool(tool) {
            return Ok(SafetyDecision::Allow);
        }

        // Fail-Safe: bash tools without a parseable `command` are refused
        // rather than passed through. Returning a generic reason avoids
        // leaking the malformed args back into a model-visible error.
        let command = match args.get("command").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return Ok(SafetyDecision::Block {
                    reason: "bash tool call missing required 'command' string field".to_string(),
                });
            }
        };

        match validate_command(command, self.permission_mode, &self.workspace) {
            ValidationResult::Allow => Ok(SafetyDecision::Allow),
            ValidationResult::Block { reason } => Ok(SafetyDecision::Block { reason }),
            ValidationResult::Warn { message } => {
                // TODO(#73): final Warn handling is a red-line decision.
                // For now, log and allow so callers wiring this scaffold up
                // get explicit signal without a behaviour change.
                tracing::warn!(
                    tool,
                    %message,
                    "BashValidationHook Warn (TODO #73: final policy pending)"
                );
                Ok(SafetyDecision::Allow)
            }
        }
    }

    async fn after_tool_output(
        &self,
        _tool: &str,
        _output: &mut String,
    ) -> Result<(), SafetyError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hook(mode: PermissionMode) -> BashValidationHook {
        BashValidationHook::new(mode, PathBuf::from("/workspace"))
    }

    #[tokio::test]
    async fn non_bash_tool_short_circuits_to_allow() {
        let h = hook(PermissionMode::ReadOnly);
        let mut args = json!({ "path": "/etc/passwd" });
        let decision = h
            .before_tool_call("read_file", &mut args)
            .await
            .expect("hook should not error");
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn bash_tool_with_safe_command_is_allowed() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "command": "ls -la" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn bash_tool_missing_command_field_is_blocked_fail_safe() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "not_command": "ls" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        match decision {
            SafetyDecision::Block { reason } => {
                assert!(
                    reason.contains("missing"),
                    "expected Fail-Safe reason mentioning missing field, got: {reason}"
                );
            }
            other => panic!("expected Block (Fail-Safe), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bash_tool_empty_command_field_is_blocked_fail_safe() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "command": "" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert!(matches!(decision, SafetyDecision::Block { .. }));
    }

    #[tokio::test]
    async fn destructive_command_in_read_only_mode_is_blocked() {
        let h = hook(PermissionMode::ReadOnly);
        let mut args = json!({ "command": "rm -rf /tmp/secret-payload.bin" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        match decision {
            SafetyDecision::Block { reason } => {
                // Safety-audit: the block reason describes the command class,
                // not the original argument list. The full argument path
                // (which may contain secrets) must NOT appear verbatim.
                assert!(
                    !reason.contains("/tmp/secret-payload.bin"),
                    "block reason leaked original command path: {reason}"
                );
            }
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_command_in_read_only_mode_is_blocked() {
        let h = hook(PermissionMode::ReadOnly);
        let mut args = json!({ "command": "cp src.txt dst.txt" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert!(matches!(decision, SafetyDecision::Block { .. }));
    }

    #[tokio::test]
    async fn warn_path_currently_returns_allow_pending_issue_73() {
        // `rm -rf /` triggers a destructive Warn (not Block) under
        // WorkspaceWrite mode. The scaffold's TODO(#73) policy is to log
        // and allow; final Warn handling is an ADR-redline decision.
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert_eq!(
            decision,
            SafetyDecision::Allow,
            "Warn path is currently Allow + log (see TODO #73)"
        );
    }

    #[tokio::test]
    async fn custom_tool_name_override_is_respected() {
        let h = BashValidationHook::new(PermissionMode::ReadOnly, PathBuf::from("/workspace"))
            .with_tool_names(["my_custom_shell"]);
        // Default name `bash` should no longer match.
        let mut args = json!({ "command": "rm file" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert_eq!(
            decision,
            SafetyDecision::Allow,
            "after override, 'bash' is no longer recognised"
        );

        // The overridden name should match.
        let mut args = json!({ "command": "rm file" });
        let decision = h
            .before_tool_call("my_custom_shell", &mut args)
            .await
            .expect("hook should not error");
        assert!(
            matches!(decision, SafetyDecision::Block { .. }),
            "overridden tool name should run the validator"
        );
    }

    #[tokio::test]
    async fn pass_through_methods_are_inert() {
        let h = hook(PermissionMode::ReadOnly);

        let mut prompt = "hello".to_string();
        assert_eq!(
            h.before_prompt(&mut prompt)
                .await
                .expect("before_prompt should not error"),
            SafetyDecision::Allow
        );
        assert_eq!(prompt, "hello", "before_prompt must not mutate");

        let mut completion = "world".to_string();
        h.after_completion(&mut completion)
            .await
            .expect("after_completion should not error");
        assert_eq!(completion, "world", "after_completion must not mutate");

        let mut output = "tool output".to_string();
        h.after_tool_output("bash", &mut output)
            .await
            .expect("after_tool_output should not error");
        assert_eq!(output, "tool output", "after_tool_output must not mutate");
    }
}
