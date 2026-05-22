//! `dasclaw_core::EgressGate` adapter for [`SafetyLayer`] (ADR-148).
//!
//! This module is gated behind the `egress-gate` feature so `dasclaw_safety`
//! stays dependency-free for its original callers (HTTP middleware, inbound
//! message scanners). When the feature is on, the agent runtime can plug
//! [`IronclawEgressGate`] into the single-method egress contract.
//!
//! # Mapping (single hook → multi-kind dispatch)
//!
//! | `EgressKind`              | `SafetyLayer` call                        | Decision rule |
//! |---------------------------|-------------------------------------------|---------------|
//! | `LlmRequest`              | `scan_inbound_for_secrets` + `leak_detector` | Secret detected → `Block` (fail-safe — never ship secrets to LLM). |
//! | `ToolExecution { tool }`  | `validator().validate_tool_params`         | Invalid args → `Block`. JSON parse failure → `Block` (fail-safe). |
//! | `UserDisplay`             | `leak_detector().scan_and_clean`           | Mutation → `Redact { sanitized, stats }`; scan error → `Block`. |
//! | `Persistence`             | `leak_detector().scan_and_clean`           | Same as `UserDisplay` (full scan). |
//!
//! All paths are **Fail-Safe**: on internal error we return `Block`, never
//! `Allow`. Composition with bash gates is done at the caller via
//! [`dasclaw_core::CompositeEgressGate`].

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::{EgressDecision, EgressGate, EgressKind, RedactionStats};
use serde_json::Value;

use crate::SafetyLayer;

/// Adapter that exposes [`SafetyLayer`] as an `dasclaw_core::EgressGate`.
///
/// `Arc` is used so the same layer can be shared across the agent runtime,
/// background jobs, and HTTP middleware without duplicating state.
#[derive(Clone)]
pub struct IronclawEgressGate {
    layer: Arc<SafetyLayer>,
}

impl IronclawEgressGate {
    pub fn new(layer: Arc<SafetyLayer>) -> Self {
        Self { layer }
    }

    pub fn layer(&self) -> &SafetyLayer {
        &self.layer
    }

    /// Apply the leak-detector path used by `UserDisplay` and `Persistence`.
    /// Returns `Allow` when nothing changed, `Redact` when scrubbed, `Block`
    /// when the detector itself fails (fail-safe).
    fn redact_via_leak_detector(&self, body: &str) -> EgressDecision {
        match self.layer.leak_detector().scan_and_clean(body) {
            Ok(cleaned) => {
                if cleaned == body {
                    EgressDecision::Allow
                } else {
                    EgressDecision::Redact {
                        sanitized: cleaned,
                        stats: RedactionStats::default(),
                    }
                }
            }
            Err(_) => EgressDecision::Block {
                reason: "leak detector failed; refusing to release potentially-leaky payload"
                    .to_string(),
                stats: RedactionStats::default(),
            },
        }
    }
}

#[async_trait]
impl EgressGate for IronclawEgressGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        match kind {
            EgressKind::LlmRequest => {
                // Fail-safe: secret heading to the LLM must never go out.
                if let Some(reason) = self.layer.scan_inbound_for_secrets(payload) {
                    return EgressDecision::Block {
                        reason,
                        stats: RedactionStats::default(),
                    };
                }
                // Leak detector double-check on the outbound prompt body.
                self.redact_via_leak_detector(payload)
            }
            EgressKind::ToolExecution { tool: _ } => {
                let args: Value = match serde_json::from_str(payload) {
                    Ok(v) => v,
                    Err(err) => {
                        return EgressDecision::Block {
                            reason: format!(
                                "ironclaw_egress: tool payload was not valid JSON: {err}"
                            ),
                            stats: RedactionStats::default(),
                        };
                    }
                };
                let result = self.layer.validator().validate_tool_params(&args);
                if result.is_valid {
                    EgressDecision::Allow
                } else {
                    let reason = result
                        .errors
                        .iter()
                        .map(|e| format!("{e:?}"))
                        .collect::<Vec<_>>()
                        .join("; ");
                    EgressDecision::Block {
                        reason: if reason.is_empty() {
                            "tool parameters failed validation".to_string()
                        } else {
                            reason
                        },
                        stats: RedactionStats::default(),
                    }
                }
            }
            EgressKind::UserDisplay | EgressKind::Persistence => {
                self.redact_via_leak_detector(payload)
            }
            // `EgressKind` is `#[non_exhaustive]`. Any future kind (e.g.
            // background telemetry) is unknown to this gate and MUST
            // Fail-Safe to `Block` rather than silently allow egress
            // past the leak/secret scanners (AGENTS.md red line:
            // "安全功能要 Fail-Safe，不允许 Fail-Open"). When a new
            // `EgressKind` lands, this arm must be replaced with an
            // explicit handler before the gate ships.
            _ => EgressDecision::Block {
                reason: "ironclaw_egress: unsupported EgressKind, refusing egress (fail-safe)"
                    .to_string(),
                stats: RedactionStats::default(),
            },
        }
    }
}

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
    async fn llm_request_allows_clean_prompt() {
        let gate = IronclawEgressGate::new(layer());
        let decision = gate
            .check(&EgressKind::LlmRequest, "hello, please list files")
            .await;
        assert_eq!(decision, EgressDecision::Allow);
    }

    #[tokio::test]
    async fn llm_request_blocks_prompt_with_openai_key() {
        let gate = IronclawEgressGate::new(layer());
        let payload = format!("use this key: sk-{}", "A".repeat(48));
        let decision = gate.check(&EgressKind::LlmRequest, &payload).await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "expected Block, got {decision:?}",
        );
    }

    #[tokio::test]
    async fn user_display_redacts_secret() {
        let gate = IronclawEgressGate::new(layer());
        let payload = format!("the API key is sk-{}", "A".repeat(48));
        let decision = gate.check(&EgressKind::UserDisplay, &payload).await;
        match decision {
            EgressDecision::Redact { sanitized, .. } => {
                assert_ne!(sanitized, payload, "secret must be scrubbed");
            }
            EgressDecision::Block { .. } => {
                // Acceptable fail-safe path if leak detector returns Err.
            }
            other => panic!("expected Redact/Block, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn user_display_passes_clean_text_through() {
        let gate = IronclawEgressGate::new(layer());
        let decision = gate.check(&EgressKind::UserDisplay, "All done.").await;
        assert_eq!(decision, EgressDecision::Allow);
    }

    #[tokio::test]
    async fn tool_execution_allows_clean_args() {
        let gate = IronclawEgressGate::new(layer());
        let payload = json!({"path": "/tmp/foo.txt"}).to_string();
        let decision = gate
            .check(
                &EgressKind::ToolExecution {
                    tool: "write_file".to_string(),
                },
                &payload,
            )
            .await;
        assert_eq!(decision, EgressDecision::Allow);
    }

    #[tokio::test]
    async fn tool_execution_blocks_invalid_json_fail_safe() {
        let gate = IronclawEgressGate::new(layer());
        let decision = gate
            .check(
                &EgressKind::ToolExecution {
                    tool: "write_file".to_string(),
                },
                "this is not json",
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "malformed JSON must Fail-Safe Block; got {decision:?}",
        );
    }

    #[tokio::test]
    async fn persistence_redacts_secret() {
        let gate = IronclawEgressGate::new(layer());
        let payload = format!("here is the token: sk-{}", "B".repeat(48));
        let decision = gate.check(&EgressKind::Persistence, &payload).await;
        assert!(
            matches!(
                decision,
                EgressDecision::Redact { .. } | EgressDecision::Block { .. }
            ),
            "secret must be redacted or blocked, got {decision:?}",
        );
    }
}
