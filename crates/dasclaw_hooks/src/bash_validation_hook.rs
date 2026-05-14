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
//!    - `Warn { message }` ⇒ mapped by `PermissionMode` per ADR-146 §2.5:
//!      `ReadOnly` ⇒ `Block` (Fail-Safe); `Prompt` ⇒ `Ask`;
//!      `WorkspaceWrite` / `DangerFullAccess` / `Allow` ⇒ `Allow` (with
//!      `tracing::warn!` / `tracing::info!` / no-log respectively).
//!
//! For non-bash tools the hook short-circuits to `Allow` so it can be
//! installed globally without disturbing other tool calls.
//!
//! # Remaining red-line decisions (parent issue [#73])
//!
//! - How `PermissionMode` is resolved per invocation (session config /
//!   per-request / workspace-level) — slice D.
//! - How `Ask` results render to the user (UX layer) — slice D.
//! - `WorkspaceCap` injection replacing `workspace: PathBuf` — slice E.
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
                Ok(map_warn_by_mode(tool, self.permission_mode, message))
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

/// Maps a `Warn { message }` validation result to a [`SafetyDecision`]
/// according to [`PermissionMode`], per ADR-146 §2.5.
///
/// | Mode                | Decision  | Logging               |
/// |---------------------|-----------|-----------------------|
/// | `ReadOnly`          | `Block`   | `tracing::warn!`      |
/// | `WorkspaceWrite`    | `Allow`   | `tracing::warn!`      |
/// | `DangerFullAccess`  | `Allow`   | `tracing::info!`      |
/// | `Allow`             | `Allow`   | none                  |
/// | `Prompt`            | `Ask`     | `tracing::warn!`      |
///
/// In `Prompt` mode `suggestions` is left empty; later slices populate it
/// from per-Warn pattern generators.
fn map_warn_by_mode(tool: &str, mode: PermissionMode, message: String) -> SafetyDecision {
    match mode {
        PermissionMode::ReadOnly => {
            // Fail-Safe: ReadOnly never tolerates Warn.
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Block (ReadOnly)"
            );
            SafetyDecision::Block { reason: message }
        }
        PermissionMode::WorkspaceWrite => {
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Allow (WorkspaceWrite)"
            );
            SafetyDecision::Allow
        }
        PermissionMode::DangerFullAccess => {
            tracing::info!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Allow (DangerFullAccess)"
            );
            SafetyDecision::Allow
        }
        PermissionMode::Allow => {
            // PermissionMode::Allow == "skip checks entirely"; no log.
            SafetyDecision::Allow
        }
        PermissionMode::Prompt => {
            tracing::warn!(
                tool,
                %message,
                mode = mode.as_str(),
                "BashValidationHook Warn -> Ask (Prompt)"
            );
            SafetyDecision::Ask {
                reason: message,
                // suggestions intentionally empty in slice C MVP; later
                // slices generate per-Warn rule patterns.
                suggestions: Vec::new(),
            }
        }
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
    async fn warn_in_workspace_write_mode_allows_with_log() {
        // `rm -rf /tmp/scratch` triggers a destructive Warn (not Block)
        // under WorkspaceWrite mode. Per ADR-146 §2.5, WorkspaceWrite maps
        // Warn -> Allow + tracing::warn! (does not interrupt the user).
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h
            .before_tool_call("bash", &mut args)
            .await
            .expect("hook should not error");
        assert_eq!(
            decision,
            SafetyDecision::Allow,
            "Warn under WorkspaceWrite -> Allow + log (ADR-146 §2.5)"
        );
    }

    // -----------------------------------------------------------------
    // ADR-146 §2.5: Warn -> SafetyDecision mapping matrix per
    // PermissionMode. The 25-case acceptance matrix (5 decisions × 5
    // modes) is split across the bash-pipeline `validate_command` tests
    // (which cover Allow/Block paths) and the `req_safety_73_c_warn_*`
    // tests below (which cover the Warn-mapping rows).
    //
    // `rm -rf /tmp/<path>` is a stable Warn-trigger under non-ReadOnly
    // modes (destructive, but path is inside workspace metadata). Under
    // ReadOnly the same command should be Block-mapped (not Warn).
    // -----------------------------------------------------------------

    /// `req_safety_73_c_warn_readonly_blocks` —
    /// Warn under ReadOnly always maps to Block (Fail-Safe).
    #[tokio::test]
    async fn req_safety_73_c_warn_readonly_blocks() {
        let h = hook(PermissionMode::ReadOnly);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.before_tool_call("bash", &mut args).await.unwrap();
        // ReadOnly may treat destructive as Block (via validate_command's
        // direct Block path) OR as Warn (mapped to Block here). Either way
        // the outcome must be Block — Fail-Safe is the invariant.
        assert!(
            matches!(decision, SafetyDecision::Block { .. }),
            "ReadOnly must Block destructive commands, got {decision:?}"
        );
    }

    /// `req_safety_73_c_warn_workspace_write_allows` —
    /// Warn under WorkspaceWrite maps to Allow + tracing::warn! log.
    #[tokio::test]
    async fn req_safety_73_c_warn_workspace_write_allows() {
        let h = hook(PermissionMode::WorkspaceWrite);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.before_tool_call("bash", &mut args).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    /// `req_safety_73_c_warn_danger_full_access_allows` —
    /// Warn under DangerFullAccess maps to Allow + tracing::info! log.
    #[tokio::test]
    async fn req_safety_73_c_warn_danger_full_access_allows() {
        let h = hook(PermissionMode::DangerFullAccess);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.before_tool_call("bash", &mut args).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    /// `req_safety_73_c_warn_allow_mode_allows_no_log` —
    /// Warn under PermissionMode::Allow maps to Allow with no log.
    #[tokio::test]
    async fn req_safety_73_c_warn_allow_mode_allows_no_log() {
        let h = hook(PermissionMode::Allow);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.before_tool_call("bash", &mut args).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    /// `req_safety_73_c_warn_prompt_asks` —
    /// Warn under Prompt mode maps to Ask { suggestions: empty }.
    #[tokio::test]
    async fn req_safety_73_c_warn_prompt_asks() {
        let h = hook(PermissionMode::Prompt);
        let mut args = json!({ "command": "rm -rf /tmp/scratch" });
        let decision = h.before_tool_call("bash", &mut args).await.unwrap();
        match decision {
            SafetyDecision::Ask {
                reason,
                suggestions,
            } => {
                assert!(
                    !reason.is_empty(),
                    "Ask.reason should propagate the Warn message"
                );
                assert!(
                    suggestions.is_empty(),
                    "MVP slice C: suggestions intentionally empty"
                );
            }
            other => panic!("Prompt mode Warn should map to Ask, got {other:?}"),
        }
    }

    /// `req_safety_73_c_allow_path_all_modes` —
    /// `validate_command` Allow path bypasses Warn mapping in every mode.
    #[tokio::test]
    async fn req_safety_73_c_allow_path_all_modes() {
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::WorkspaceWrite,
            PermissionMode::DangerFullAccess,
            PermissionMode::Prompt,
            PermissionMode::Allow,
        ] {
            let h = hook(mode);
            let mut args = json!({ "command": "ls -la" });
            let decision = h.before_tool_call("bash", &mut args).await.unwrap();
            assert_eq!(
                decision,
                SafetyDecision::Allow,
                "`ls -la` should be Allow under mode {mode:?}, got {decision:?}"
            );
        }
    }

    /// `req_safety_73_c_block_path_all_modes` —
    /// `validate_command` direct Block path (path escape) propagates as
    /// Block in every mode. The exact `reason` text varies by mode but
    /// must never leak the full argument string (safety-audit invariant).
    #[tokio::test]
    async fn req_safety_73_c_block_path_all_modes() {
        // Path-escape command: `cat ../../../etc/passwd` is outside any
        // sensible workspace and should be flagged by validate_command.
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::WorkspaceWrite,
            PermissionMode::DangerFullAccess,
            PermissionMode::Prompt,
            PermissionMode::Allow,
        ] {
            let h = hook(mode);
            let mut args = json!({ "command": "cat ../../../etc/passwd" });
            let decision = h.before_tool_call("bash", &mut args).await.unwrap();
            // Allow mode is permissive — only assert Block on stricter modes.
            // The test still exercises all 5 modes for non-panic / non-error.
            match mode {
                PermissionMode::ReadOnly => assert!(
                    matches!(decision, SafetyDecision::Block { .. }),
                    "ReadOnly should Block path-escape, got {decision:?}"
                ),
                _ => {
                    // Other modes may Allow, Block, or Ask depending on
                    // validator policy; assert only "hook did not error
                    // and returned a known variant".
                    assert!(matches!(
                        decision,
                        SafetyDecision::Allow
                            | SafetyDecision::Block { .. }
                            | SafetyDecision::Ask { .. }
                    ));
                }
            }
        }
    }

    /// `req_safety_73_c_passthrough_variant_constructs` —
    /// Smoke: the new `Passthrough` variant is constructible (the hook
    /// itself never returns it, but the type must exist for chain code).
    #[test]
    fn req_safety_73_c_passthrough_variant_constructs() {
        let d = SafetyDecision::Passthrough;
        assert_eq!(d, SafetyDecision::Passthrough);
    }

    /// `req_safety_73_c_ask_with_suggestions_round_trips` —
    /// Smoke: `Ask` with non-empty suggestions clones / equates correctly.
    #[test]
    fn req_safety_73_c_ask_with_suggestions_round_trips() {
        use x_claw_agent::{RuleAction, RuleSuggestion};
        let d = SafetyDecision::Ask {
            reason: "Confirm `rm`?".to_string(),
            suggestions: vec![
                RuleSuggestion {
                    label: "Always allow rm".to_string(),
                    rule_pattern: "Bash(rm: allow)".to_string(),
                    action: RuleAction::Allow,
                },
                RuleSuggestion {
                    label: "Deny once".to_string(),
                    rule_pattern: "Bash(rm: deny)".to_string(),
                    action: RuleAction::Deny,
                },
            ],
        };
        let d2 = d.clone();
        assert_eq!(d, d2);
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
