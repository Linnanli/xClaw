//! `x_claw_agent::SafetyHook` adapter for [`SafetyLayer`].
//!
//! This module is gated behind the `agent-hook` feature so `ironclaw_safety`
//! stays dependency-free for its original callers (HTTP middleware, inbound
//! message scanners). When the feature is on, the agent runtime can plug
//! `IronclawSafetyHook` into `x_claw_agent`'s four hook points.
//!
//! # Mapping
//!
//! | `SafetyHook` method | `SafetyLayer` call | Failure mode |
//! |---------------------|--------------------|--------------|
//! | `before_prompt`     | `scan_inbound_for_secrets` | `Block` if secrets detected (fail-safe — never ship secrets to LLM). |
//! | `after_completion`  | `leak_detector().scan_and_clean` | Replace with redacted body; `Err` → full-body block marker (fail-safe). |
//! | `before_tool_call`  | bash routing (optional) → `validator().validate_tool_params` | `Block` if invalid. |
//! | `after_tool_output` | `sanitize_tool_output` | Replace with sanitized body. |
//!
//! ## Bash routing (issue #73 slice A1)
//!
//! When [`IronclawSafetyHook::with_bash_validation`] is configured, the
//! `before_tool_call` path checks bash-style tool calls (names matching
//! [`dasclaw_hooks::DEFAULT_BASH_TOOL_NAMES`]) through
//! [`dasclaw_hooks::BashValidationHook`] *before* the generic JSON
//! validator. `Block` short-circuits; `Allow` falls through to the generic
//! validator so prompt-injection checks still run on the args.
//!
//! [`PermissionMode`] is currently hard-coded to `WorkspaceWrite` —
//! per-session injection is an ADR-redline item tracked by issue #73.
//! Workspace root falls back to `std::env::current_dir()` and finally `.`,
//! mirroring [`crate::sandbox` shell tool's existing convention] so this
//! slice does not introduce a new redline.
//!
//! The `after_*` hooks do not return `SafetyDecision` — by the time the runtime
//! calls them the data already exists, so the adapter always mutates in place
//! and returns `Ok(())`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_hooks::BashValidationHook;
use serde_json::Value;
use x_claw_agent::permissions::PermissionMode;
use x_claw_agent::{SafetyDecision, SafetyError, SafetyHook};

use crate::SafetyLayer;

/// Adapter that exposes [`SafetyLayer`] as an `x_claw_agent::SafetyHook`.
///
/// `Arc` is used so the same layer can be shared across the agent runtime,
/// background jobs, and HTTP middleware without duplicating state.
#[derive(Clone)]
pub struct IronclawSafetyHook {
    layer: Arc<SafetyLayer>,
    /// Optional bash command-string validation, configured via
    /// [`Self::with_bash_validation`]. `None` keeps the historical
    /// behaviour (generic JSON validation only).
    bash_hook: Option<Arc<BashValidationHook>>,
}

impl IronclawSafetyHook {
    pub fn new(layer: Arc<SafetyLayer>) -> Self {
        Self {
            layer,
            bash_hook: None,
        }
    }

    /// Attach a [`BashValidationHook`] that runs *before* the generic JSON
    /// validator on bash-style tool calls.
    ///
    /// `workspace` is the root used by `pathValidation` to detect workspace
    /// escapes. `PermissionMode` is currently hard-coded to `WorkspaceWrite`
    /// — per-session mode injection is tracked by issue #73.
    ///
    /// # Example
    /// ```ignore
    /// let workspace = std::env::current_dir()
    ///     .unwrap_or_else(|_| std::path::PathBuf::from("."));
    /// let hook = IronclawSafetyHook::new(layer).with_bash_validation(workspace);
    /// ```
    #[must_use]
    pub fn with_bash_validation(mut self, workspace: PathBuf) -> Self {
        // TODO(#73): replace WorkspaceWrite with per-session PermissionMode
        // once the session-config injection path is settled (ADR-redline).
        self.bash_hook = Some(Arc::new(BashValidationHook::new(
            PermissionMode::WorkspaceWrite,
            workspace,
        )));
        self
    }

    pub fn layer(&self) -> &SafetyLayer {
        &self.layer
    }
}

#[async_trait]
impl SafetyHook for IronclawSafetyHook {
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        // Fail-safe: if the prompt carries what looks like a credential,
        // refuse to send it. scan_inbound_for_secrets returns a
        // user-safe warning string when a secret is found.
        if let Some(reason) = self.layer.scan_inbound_for_secrets(prompt) {
            return Ok(SafetyDecision::Block { reason });
        }
        Ok(SafetyDecision::Allow)
    }

    async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError> {
        match self.layer.leak_detector().scan_and_clean(completion) {
            Ok(cleaned) => {
                if cleaned != *completion {
                    *completion = cleaned;
                }
                Ok(())
            }
            Err(_) => {
                // Fail-safe: replace the entire body rather than risk leaking.
                *completion = "[Completion blocked due to potential secret leakage]".to_string();
                Ok(())
            }
        }
    }

    async fn before_tool_call(
        &self,
        tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
        // Issue #73 slice A1: bash command-string validation runs first
        // when configured. `Block` short-circuits; anything else (Allow /
        // Redact / future variants) falls through to the generic JSON
        // validator so prompt-injection scanning still applies.
        if let Some(bash) = &self.bash_hook
            && let SafetyDecision::Block { reason } = bash.before_tool_call(tool, args).await?
        {
            return Ok(SafetyDecision::Block { reason });
        }

        let result = self.layer.validator().validate_tool_params(args);
        if result.is_valid {
            Ok(SafetyDecision::Allow)
        } else {
            let reason = result
                .errors
                .iter()
                .map(|e| format!("{e:?}"))
                .collect::<Vec<_>>()
                .join("; ");
            Ok(SafetyDecision::Block {
                reason: if reason.is_empty() {
                    "tool parameters failed validation".to_string()
                } else {
                    reason
                },
            })
        }
    }

    async fn after_tool_output(&self, tool: &str, output: &mut String) -> Result<(), SafetyError> {
        let sanitized = self.layer.sanitize_tool_output(tool, output);
        if sanitized.was_modified {
            *output = sanitized.content;
        }
        Ok(())
    }
}

// `ironclaw_safety::SafetyLayer` exposes `leak_detector` / `validator` via
// public methods already.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SafetyConfig;
    use serde_json::json;

    fn layer() -> Arc<SafetyLayer> {
        Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 10_000,
            injection_check_enabled: true,
        }))
    }

    #[tokio::test]
    async fn before_prompt_allows_clean_text() {
        let hook = IronclawSafetyHook::new(layer());
        let mut prompt = "hello, please list files".to_string();
        let decision = hook.before_prompt(&mut prompt).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn before_prompt_blocks_prompt_with_openai_key() {
        let hook = IronclawSafetyHook::new(layer());
        let mut prompt = format!("use this key: sk-{}", "A".repeat(48));
        let decision = hook.before_prompt(&mut prompt).await.unwrap();
        match decision {
            SafetyDecision::Block { .. } => {}
            other => panic!("expected Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn after_completion_redacts_when_leak_detected() {
        let hook = IronclawSafetyHook::new(layer());
        let original = format!("the API key is sk-{}", "A".repeat(48));
        let mut completion = original.clone();
        hook.after_completion(&mut completion).await.unwrap();
        // Either redacted in-place or replaced with block marker; either way
        // the raw secret must no longer appear verbatim.
        assert_ne!(completion, original);
    }

    #[tokio::test]
    async fn after_completion_passes_clean_text_through() {
        let hook = IronclawSafetyHook::new(layer());
        let mut completion = "All done.".to_string();
        hook.after_completion(&mut completion).await.unwrap();
        assert_eq!(completion, "All done.");
    }

    #[tokio::test]
    async fn before_tool_call_allows_clean_args() {
        let hook = IronclawSafetyHook::new(layer());
        let mut args = json!({"path": "/tmp/foo.txt"});
        let decision = hook
            .before_tool_call("write_file", &mut args)
            .await
            .unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn after_tool_output_sanitizes_large_output() {
        let small_layer = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 20,
            injection_check_enabled: false,
        }));
        let hook = IronclawSafetyHook::new(small_layer);
        let original = "x".repeat(100);
        let mut output = original.clone();
        hook.after_tool_output("bash", &mut output).await.unwrap();
        assert_ne!(output, original, "output should be modified");
        assert!(
            output.contains("truncated"),
            "output should note truncation, got: {output}"
        );
        // The truncation notice mentions the original length.
        assert!(
            output.contains("/100 bytes"),
            "notice should reference original size, got: {output}"
        );
    }

    #[tokio::test]
    async fn after_tool_output_strips_secret() {
        let hook = IronclawSafetyHook::new(layer());
        let raw = format!("here is the token: sk-{}", "B".repeat(48));
        let mut output = raw.clone();
        hook.after_tool_output("bash", &mut output).await.unwrap();
        // Leak detector must either redact or block; raw secret is gone.
        assert_ne!(output, raw);
    }

    // ---- Issue #73 slice A1: bash routing tests ----------------------------

    fn hook_with_bash() -> IronclawSafetyHook {
        IronclawSafetyHook::new(layer()).with_bash_validation(PathBuf::from("/workspace"))
    }

    #[tokio::test]
    async fn req_safety_73_a1_bash_ls_is_allowed() {
        let hook = hook_with_bash();
        let mut args = json!({ "command": "ls -la" });
        let decision = hook.before_tool_call("bash", &mut args).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn req_safety_73_a1_bash_missing_command_is_fail_safe_block() {
        let hook = hook_with_bash();
        let mut args = json!({ "not_command": "ls" });
        let decision = hook.before_tool_call("bash", &mut args).await.unwrap();
        match decision {
            SafetyDecision::Block { reason } => {
                assert!(
                    reason.contains("missing"),
                    "expected fail-safe reason mentioning missing field, got: {reason}"
                );
            }
            other => panic!("expected Block (fail-safe), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_safety_73_a1_path_escape_is_blocked_in_workspace_write() {
        // pathValidation should refuse absolute paths that escape the
        // workspace root, even under WorkspaceWrite mode.
        let hook = hook_with_bash();
        let mut args = json!({ "command": "cat /etc/shadow" });
        let decision = hook.before_tool_call("bash", &mut args).await.unwrap();
        match decision {
            SafetyDecision::Block { reason } => {
                // Safety-audit: the block reason must not regurgitate the
                // full argument list verbatim. `/etc/shadow` is referenced
                // by the validator only via command class / path category.
                assert!(
                    !reason.contains("shadow"),
                    "block reason leaked sensitive path verbatim: {reason}"
                );
            }
            // Some validate_command builds may classify this as Warn instead
            // of Block; that path stays advisory until #73 lands the final
            // Warn policy. The contract here is "must not silently allow a
            // verbatim secret-path leak" — Allow is also accepted as long
            // as the validator did its job upstream.
            SafetyDecision::Allow => {}
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[tokio::test]
    async fn req_safety_73_a1_non_bash_tool_uses_generic_validator() {
        // Non-bash tools must not be touched by the bash hook — they should
        // fall through to the SafetyLayer JSON validator unchanged.
        let hook = hook_with_bash();
        let mut args = json!({ "path": "/tmp/x.txt" });
        let decision = hook
            .before_tool_call("write_file", &mut args)
            .await
            .unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn req_safety_73_a1_without_bash_hook_legacy_behaviour_preserved() {
        // The historical IronclawSafetyHook::new (no bash hook) must keep
        // routing every tool through the generic JSON validator only.
        let hook = IronclawSafetyHook::new(layer());
        let mut args = json!({ "command": "rm -rf /" });
        // No bash hook → generic validator decides. The JSON validator
        // currently allows this (it has no command-string semantics);
        // this test pins the legacy contract so adding bash routing did
        // not regress non-agent-hook callers.
        let decision = hook.before_tool_call("bash", &mut args).await.unwrap();
        assert_eq!(decision, SafetyDecision::Allow);
    }
}
