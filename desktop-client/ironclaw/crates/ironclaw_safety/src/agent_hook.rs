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
//! | `before_tool_call`  | `validator().validate_tool_params` | `Block` if invalid. |
//! | `after_tool_output` | `sanitize_tool_output` | Replace with sanitized body. |
//!
//! The `after_*` hooks do not return `SafetyDecision` — by the time the runtime
//! calls them the data already exists, so the adapter always mutates in place
//! and returns `Ok(())`.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use x_claw_agent::{SafetyDecision, SafetyError, SafetyHook};

use crate::SafetyLayer;

/// Adapter that exposes [`SafetyLayer`] as an `x_claw_agent::SafetyHook`.
///
/// `Arc` is used so the same layer can be shared across the agent runtime,
/// background jobs, and HTTP middleware without duplicating state.
#[derive(Clone)]
pub struct IronclawSafetyHook {
    layer: Arc<SafetyLayer>,
}

impl IronclawSafetyHook {
    pub fn new(layer: Arc<SafetyLayer>) -> Self {
        Self { layer }
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
        _tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
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
}
