//! `CompositeSafetyHook` — chain multiple [`SafetyHook`] implementations
//! into a single hook (per ADR-147).
//!
//! ## Why this exists
//!
//! `HookBundle.safety` is a single `Arc<dyn SafetyHook>` slot. Composing
//! several distinct safety concerns (bash command validation, generic
//! JSON validation, project-level rules, MCP permission gates, …) used
//! to require ad-hoc fields like
//! `IronclawSafetyHook { bash_hook: Option<Arc<…>> }`. That's a patch-style
//! workaround that doesn't extend to N hooks.
//!
//! `CompositeSafetyHook` replaces the ad-hoc pattern with a typed chain
//! built via a builder, while preserving the type identity of
//! `Arc<dyn SafetyHook>` so the existing `HookBundle.safety` slot is
//! reused unchanged.
//!
//! ## Short-circuit semantics
//!
//! See ADR-147 §2.2 / §2.3 for the canonical specification. Quick recap:
//!
//! - `before_tool_call` / `before_prompt`:
//!   - `Allow`              → continue (record as "last allow")
//!   - `Redact`             → continue (caller already mutated payload)
//!   - `Passthrough`        → continue (do not record allow)
//!   - `Block { reason }`   → **short-circuit** return Block
//!   - `Ask { … }`          → **short-circuit** return Ask
//!   - `Err(e)`             → **short-circuit** return Err (Fail-Safe)
//! - `after_completion` / `after_tool_output`: every hook runs in order
//!   (sanitisation pipeline). Errors short-circuit; Block/Ask are
//!   meaningless on already-produced output and are tracing::error!-logged
//!   then ignored.
//!
//! ## End-of-chain rule (ADR-147 §2.2)
//!
//! If all hooks returned `Passthrough`, the chain returns `Allow` (a
//! single-hook chain of one Passthrough also yields Allow). At least one
//! non-Passthrough hook is expected at the end of the chain — currently a
//! convention enforced by builder review, not by the type system.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::hooks::{SafetyDecision, SafetyError, SafetyHook};

/// Stable identifier for a hook slot in the chain. Used in tracing logs
/// and audit records to attribute decisions to a specific hook.
pub type HookId = &'static str;

/// Sequential composition of [`SafetyHook`] implementations.
///
/// Construct via [`CompositeSafetyHook::builder`].
pub struct CompositeSafetyHook {
    hooks: Vec<(HookId, Arc<dyn SafetyHook>)>,
}

impl CompositeSafetyHook {
    /// Create a new builder.
    #[must_use]
    pub fn builder() -> CompositeSafetyHookBuilder {
        CompositeSafetyHookBuilder { hooks: Vec::new() }
    }

    /// Number of hooks currently composed (for diagnostics / tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Returns the ordered list of `HookId`s in the chain.
    pub fn hook_ids(&self) -> Vec<HookId> {
        self.hooks.iter().map(|(id, _)| *id).collect()
    }
}

/// Builder for [`CompositeSafetyHook`].
///
/// Hooks execute in insertion order. The convention is to place the most
/// specific hook first (e.g. `bash-validation`) and the most general hook
/// last (e.g. `ironclaw-safety` JSON validator).
pub struct CompositeSafetyHookBuilder {
    hooks: Vec<(HookId, Arc<dyn SafetyHook>)>,
}

impl CompositeSafetyHookBuilder {
    /// Append a hook to the chain. `id` MUST be a stable identifier (used
    /// in tracing / audit). Duplicate ids are allowed but discouraged.
    #[must_use]
    pub fn add(mut self, id: HookId, hook: Arc<dyn SafetyHook>) -> Self {
        self.hooks.push((id, hook));
        self
    }

    /// Finalise the builder.
    #[must_use]
    pub fn build(self) -> CompositeSafetyHook {
        CompositeSafetyHook { hooks: self.hooks }
    }
}

/// Outcome category used internally to apply the §2.2 short-circuit rule
/// uniformly to `before_prompt` and `before_tool_call`.
enum BeforeFlow {
    /// Continue to the next hook; record this Allow as the chain's
    /// last-known Allow (for the end-of-chain rule).
    ContinueAllow,
    /// Continue to the next hook (Redact already mutated the payload).
    ContinueRedact,
    /// Continue to the next hook without recording an Allow.
    ContinuePassthrough,
    /// Short-circuit with the given decision.
    ShortCircuit(SafetyDecision),
}

fn classify(decision: SafetyDecision) -> BeforeFlow {
    // SafetyDecision is `#[non_exhaustive]` for downstream crates, but the
    // match here lives in the defining crate so all current variants are
    // visible. If a new variant is added, the missing-arm error MUST be
    // resolved by classifying it explicitly — defaulting to a wildcard
    // would silently downgrade a new safety variant. Fail-Safe by design.
    match decision {
        SafetyDecision::Allow => BeforeFlow::ContinueAllow,
        SafetyDecision::Redact => BeforeFlow::ContinueRedact,
        SafetyDecision::Passthrough => BeforeFlow::ContinuePassthrough,
        d @ (SafetyDecision::Block { .. } | SafetyDecision::Ask { .. }) => {
            BeforeFlow::ShortCircuit(d)
        }
    }
}

#[async_trait]
impl SafetyHook for CompositeSafetyHook {
    async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        // Track whether any hook returned a non-Passthrough Allow / Redact.
        // End-of-chain rule (ADR-147 §2.2): if all hooks Passthrough, the
        // chain returns Allow.
        let mut last_concrete: Option<SafetyDecision> = None;

        for (id, hook) in &self.hooks {
            let decision = hook.before_prompt(prompt).await.inspect_err(|e| {
                tracing::error!(hook_id = id, error = %e, "CompositeSafetyHook before_prompt error");
            })?;
            match classify(decision) {
                BeforeFlow::ContinueAllow => {
                    tracing::trace!(hook_id = id, "before_prompt: Allow, continue");
                    last_concrete = Some(SafetyDecision::Allow);
                }
                BeforeFlow::ContinueRedact => {
                    tracing::trace!(hook_id = id, "before_prompt: Redact, continue");
                    last_concrete = Some(SafetyDecision::Redact);
                }
                BeforeFlow::ContinuePassthrough => {
                    tracing::trace!(hook_id = id, "before_prompt: Passthrough");
                }
                BeforeFlow::ShortCircuit(d) => {
                    tracing::debug!(
                        hook_id = id,
                        decision = ?d,
                        "before_prompt: short-circuit"
                    );
                    return Ok(d);
                }
            }
        }

        Ok(last_concrete.unwrap_or(SafetyDecision::Allow))
    }

    async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError> {
        // Sanitisation pipeline: every hook runs. Errors short-circuit
        // (Fail-Safe). after_* contracts return () — no decision.
        for (id, hook) in &self.hooks {
            hook.after_completion(completion).await.inspect_err(|e| {
                tracing::error!(
                    hook_id = id,
                    error = %e,
                    "CompositeSafetyHook after_completion error"
                );
            })?;
        }
        Ok(())
    }

    async fn before_tool_call(
        &self,
        tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
        let mut last_concrete: Option<SafetyDecision> = None;

        for (id, hook) in &self.hooks {
            let decision = hook.before_tool_call(tool, args).await.inspect_err(|e| {
                tracing::error!(
                    hook_id = id,
                    tool,
                    error = %e,
                    "CompositeSafetyHook before_tool_call error"
                );
            })?;
            match classify(decision) {
                BeforeFlow::ContinueAllow => {
                    tracing::trace!(hook_id = id, tool, "before_tool_call: Allow, continue");
                    last_concrete = Some(SafetyDecision::Allow);
                }
                BeforeFlow::ContinueRedact => {
                    tracing::trace!(hook_id = id, tool, "before_tool_call: Redact, continue");
                    last_concrete = Some(SafetyDecision::Redact);
                }
                BeforeFlow::ContinuePassthrough => {
                    tracing::trace!(hook_id = id, tool, "before_tool_call: Passthrough");
                }
                BeforeFlow::ShortCircuit(d) => {
                    tracing::debug!(
                        hook_id = id,
                        tool,
                        decision = ?d,
                        "before_tool_call: short-circuit"
                    );
                    return Ok(d);
                }
            }
        }

        Ok(last_concrete.unwrap_or(SafetyDecision::Allow))
    }

    async fn after_tool_output(&self, tool: &str, output: &mut String) -> Result<(), SafetyError> {
        for (id, hook) in &self.hooks {
            hook.after_tool_output(tool, output)
                .await
                .inspect_err(|e| {
                    tracing::error!(
                        hook_id = id,
                        tool,
                        error = %e,
                        "CompositeSafetyHook after_tool_output error"
                    );
                })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{NoopSafetyHook, RuleSuggestion};
    use serde_json::json;
    use std::sync::Mutex;

    /// A controllable test hook that returns a pre-set decision and
    /// records every invocation into a shared Vec for ordering assertions.
    #[derive(Clone)]
    struct StubHook {
        id: HookId,
        decision: SafetyDecision,
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl StubHook {
        fn new(id: HookId, decision: SafetyDecision, calls: Arc<Mutex<Vec<&'static str>>>) -> Self {
            Self {
                id,
                decision,
                calls,
            }
        }

        fn record(&self, method: &'static str) {
            self.calls
                .lock()
                .expect("stub hook mutex poisoned")
                .push(self.id);
            // Suppress unused lint when method tracking not needed.
            let _ = method;
        }
    }

    #[async_trait]
    impl SafetyHook for StubHook {
        async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            self.record("before_prompt");
            Ok(self.decision.clone())
        }
        async fn after_completion(&self, _completion: &mut String) -> Result<(), SafetyError> {
            self.record("after_completion");
            Ok(())
        }
        async fn before_tool_call(
            &self,
            _tool: &str,
            _args: &mut Value,
        ) -> Result<SafetyDecision, SafetyError> {
            self.record("before_tool_call");
            Ok(self.decision.clone())
        }
        async fn after_tool_output(
            &self,
            _tool: &str,
            _output: &mut String,
        ) -> Result<(), SafetyError> {
            self.record("after_tool_output");
            Ok(())
        }
    }

    /// Hook that mutates the args / completion to verify Redact propagation.
    struct MutatingHook {
        id: HookId,
        replacement: String,
    }

    #[async_trait]
    impl SafetyHook for MutatingHook {
        async fn before_prompt(&self, prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            *prompt = self.replacement.clone();
            Ok(SafetyDecision::Redact)
        }
        async fn after_completion(&self, completion: &mut String) -> Result<(), SafetyError> {
            *completion = self.replacement.clone();
            Ok(())
        }
        async fn before_tool_call(
            &self,
            _tool: &str,
            args: &mut Value,
        ) -> Result<SafetyDecision, SafetyError> {
            *args = json!({ "command": self.replacement });
            Ok(SafetyDecision::Redact)
        }
        async fn after_tool_output(
            &self,
            _tool: &str,
            output: &mut String,
        ) -> Result<(), SafetyError> {
            *output = self.replacement.clone();
            Ok(())
        }
        // suppress dead_code on id; used in tracing in real impls
    }

    impl MutatingHook {
        #[allow(dead_code)]
        fn id(&self) -> HookId {
            self.id
        }
    }

    /// Hook that always errors — used to assert Fail-Safe error propagation.
    struct ErrorHook;

    #[async_trait]
    impl SafetyHook for ErrorHook {
        async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
            Err(SafetyError::Internal("boom".into()))
        }
        async fn after_completion(&self, _completion: &mut String) -> Result<(), SafetyError> {
            Err(SafetyError::Internal("boom".into()))
        }
        async fn before_tool_call(
            &self,
            _tool: &str,
            _args: &mut Value,
        ) -> Result<SafetyDecision, SafetyError> {
            Err(SafetyError::Internal("boom".into()))
        }
        async fn after_tool_output(
            &self,
            _tool: &str,
            _output: &mut String,
        ) -> Result<(), SafetyError> {
            Err(SafetyError::Internal("boom".into()))
        }
    }

    fn calls() -> Arc<Mutex<Vec<&'static str>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    fn ask_decision() -> SafetyDecision {
        SafetyDecision::Ask {
            reason: "confirm?".into(),
            suggestions: vec![RuleSuggestion {
                label: "Allow once".into(),
                rule_pattern: "Bash(rm: ask)".into(),
                action: crate::hooks::RuleAction::Ask,
            }],
        }
    }

    // ----- Builder smoke -----

    #[test]
    fn req_safety_73_b_builder_records_order_and_ids() {
        let composite = CompositeSafetyHook::builder()
            .add("first", Arc::new(NoopSafetyHook))
            .add("second", Arc::new(NoopSafetyHook))
            .build();
        assert_eq!(composite.len(), 2);
        assert_eq!(composite.hook_ids(), vec!["first", "second"]);
        assert!(!composite.is_empty());
    }

    #[test]
    fn req_safety_73_b_empty_builder_is_empty() {
        let composite = CompositeSafetyHook::builder().build();
        assert!(composite.is_empty());
        assert_eq!(composite.len(), 0);
    }

    // ----- before_tool_call short-circuit matrix (ADR-147 §2.2) -----

    #[tokio::test]
    async fn req_safety_73_b_btc_all_passthrough_returns_allow() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "a",
                Arc::new(StubHook::new(
                    "a",
                    SafetyDecision::Passthrough,
                    calls.clone(),
                )),
            )
            .add(
                "b",
                Arc::new(StubHook::new(
                    "b",
                    SafetyDecision::Passthrough,
                    calls.clone(),
                )),
            )
            .build();
        let mut args = json!({ "x": 1 });
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        assert_eq!(*calls.lock().unwrap(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_all_allow_returns_allow_and_runs_all() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "a",
                Arc::new(StubHook::new("a", SafetyDecision::Allow, calls.clone())),
            )
            .add(
                "b",
                Arc::new(StubHook::new("b", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({ "x": 1 });
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        assert_eq!(*calls.lock().unwrap(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_block_first_short_circuits() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "blocker",
                Arc::new(StubHook::new(
                    "blocker",
                    SafetyDecision::Block {
                        reason: "no".into(),
                    },
                    calls.clone(),
                )),
            )
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({});
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert!(matches!(d, SafetyDecision::Block { .. }));
        // `later` must NOT have been called.
        assert_eq!(*calls.lock().unwrap(), vec!["blocker"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_block_middle_short_circuits() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "first",
                Arc::new(StubHook::new("first", SafetyDecision::Allow, calls.clone())),
            )
            .add(
                "blocker",
                Arc::new(StubHook::new(
                    "blocker",
                    SafetyDecision::Block {
                        reason: "no".into(),
                    },
                    calls.clone(),
                )),
            )
            .add(
                "last",
                Arc::new(StubHook::new("last", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({});
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert!(matches!(d, SafetyDecision::Block { .. }));
        assert_eq!(*calls.lock().unwrap(), vec!["first", "blocker"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_ask_short_circuits() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "asker",
                Arc::new(StubHook::new("asker", ask_decision(), calls.clone())),
            )
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({});
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert!(matches!(d, SafetyDecision::Ask { .. }));
        assert_eq!(*calls.lock().unwrap(), vec!["asker"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_redact_then_block_returns_block() {
        let composite = CompositeSafetyHook::builder()
            .add(
                "redactor",
                Arc::new(MutatingHook {
                    id: "redactor",
                    replacement: "REDACTED".into(),
                }),
            )
            .add(
                "blocker",
                Arc::new(StubHook::new(
                    "blocker",
                    SafetyDecision::Block {
                        reason: "still no".into(),
                    },
                    calls(),
                )),
            )
            .build();
        let mut args = json!({ "command": "secret" });
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert!(matches!(d, SafetyDecision::Block { .. }));
        // The blocker observed the redacted args, not the original.
        assert_eq!(args, json!({ "command": "REDACTED" }));
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_redact_propagates_to_next_hook() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "redactor",
                Arc::new(MutatingHook {
                    id: "redactor",
                    replacement: "REDACTED".into(),
                }),
            )
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({ "command": "secret" });
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        // Last concrete decision is Allow; chain returns Allow.
        assert_eq!(d, SafetyDecision::Allow);
        assert_eq!(args, json!({ "command": "REDACTED" }));
        assert_eq!(*calls.lock().unwrap(), vec!["later"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_btc_error_short_circuits_fail_safe() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add("err", Arc::new(ErrorHook))
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut args = json!({});
        let result = composite.before_tool_call("t", &mut args).await;
        assert!(result.is_err());
        assert!(calls.lock().unwrap().is_empty());
    }

    // ----- before_prompt mirrors before_tool_call -----

    #[tokio::test]
    async fn req_safety_73_b_bp_block_short_circuits() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "blocker",
                Arc::new(StubHook::new(
                    "blocker",
                    SafetyDecision::Block {
                        reason: "no".into(),
                    },
                    calls.clone(),
                )),
            )
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut prompt = String::from("hello");
        let d = composite.before_prompt(&mut prompt).await.unwrap();
        assert!(matches!(d, SafetyDecision::Block { .. }));
        assert_eq!(*calls.lock().unwrap(), vec!["blocker"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_bp_redact_propagates_to_next_hook() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "redactor",
                Arc::new(MutatingHook {
                    id: "redactor",
                    replacement: "REDACTED".into(),
                }),
            )
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut prompt = String::from("secret");
        let d = composite.before_prompt(&mut prompt).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        assert_eq!(prompt, "REDACTED");
        assert_eq!(*calls.lock().unwrap(), vec!["later"]);
    }

    // ----- after_* runs every hook (no short-circuit) -----

    #[tokio::test]
    async fn req_safety_73_b_after_completion_runs_every_hook_in_order() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add(
                "a",
                Arc::new(StubHook::new("a", SafetyDecision::Allow, calls.clone())),
            )
            .add(
                "b",
                Arc::new(StubHook::new("b", SafetyDecision::Allow, calls.clone())),
            )
            .add(
                "c",
                Arc::new(StubHook::new("c", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut completion = String::from("result");
        composite.after_completion(&mut completion).await.unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn req_safety_73_b_after_tool_output_chains_redactions() {
        let composite = CompositeSafetyHook::builder()
            .add(
                "first",
                Arc::new(MutatingHook {
                    id: "first",
                    replacement: "STAGE1".into(),
                }),
            )
            .add(
                "second",
                Arc::new(MutatingHook {
                    id: "second",
                    replacement: "STAGE2".into(),
                }),
            )
            .build();
        let mut output = String::from("raw");
        composite.after_tool_output("t", &mut output).await.unwrap();
        // Last hook's mutation wins.
        assert_eq!(output, "STAGE2");
    }

    #[tokio::test]
    async fn req_safety_73_b_after_completion_error_short_circuits() {
        let calls = calls();
        let composite = CompositeSafetyHook::builder()
            .add("err", Arc::new(ErrorHook))
            .add(
                "later",
                Arc::new(StubHook::new("later", SafetyDecision::Allow, calls.clone())),
            )
            .build();
        let mut completion = String::from("x");
        let result = composite.after_completion(&mut completion).await;
        assert!(result.is_err());
        assert!(calls.lock().unwrap().is_empty());
    }

    // ----- Single-hook chain equivalence -----

    #[tokio::test]
    async fn req_safety_73_b_single_passthrough_hook_returns_allow() {
        let composite = CompositeSafetyHook::builder()
            .add(
                "only",
                Arc::new(StubHook::new("only", SafetyDecision::Passthrough, calls())),
            )
            .build();
        let mut args = json!({});
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
    }

    #[tokio::test]
    async fn req_safety_73_b_empty_chain_returns_allow() {
        let composite = CompositeSafetyHook::builder().build();
        let mut args = json!({});
        let d = composite.before_tool_call("t", &mut args).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        let mut prompt = String::new();
        let d = composite.before_prompt(&mut prompt).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        let mut completion = String::new();
        composite.after_completion(&mut completion).await.unwrap();
        let mut output = String::new();
        composite.after_tool_output("t", &mut output).await.unwrap();
    }
}
