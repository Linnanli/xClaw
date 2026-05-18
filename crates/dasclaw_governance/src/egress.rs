//! ADR-148 EgressGate — Layer B trait seam for data leaving the agent's
//! trust boundary.
//!
//! # Scope (ADR-148 §2.2)
//!
//! Replaces the four-method `SafetyHook` seam (`before_prompt`,
//! `after_completion`, `before_tool_call`, `after_tool_output`) with a
//! single `check(kind, payload) -> EgressDecision` surface keyed by the
//! [`EgressKind`] enum.
//!
//! Each kind corresponds to a direction the data is about to leave the
//! agent runtime:
//!
//! | Kind | Direction | Example |
//! |------|-----------|---------|
//! | [`EgressKind::LlmRequest`] | HTTP body for the LLM | most-recent user message before each call |
//! | [`EgressKind::ToolExecution`] | tool runtime (bash / fs / MCP) | tool args before dispatch |
//! | [`EgressKind::UserDisplay`] | UI surface (Tauri / TTY / SSE) | LLM completion before render |
//! | [`EgressKind::Persistence`] | local DB / admin-backend | conversation entry before write |
//!
//! # Fail-closed contract (ADR-148 §2.2)
//!
//! Implementations MUST translate any internal error into
//! [`EgressDecision::Block`] rather than panicking or returning a
//! "neutral" `Allow`. The trait deliberately does not carry a `Result`
//! so error escape paths cannot be silently inserted.
//!
//! # No mutation in Allow path (ADR-148 §5)
//!
//! When an implementation returns [`EgressDecision::Allow`] the payload
//! is delivered downstream **unchanged**. Sanitization is signalled via
//! [`EgressDecision::Redact`] which carries the rewritten payload —
//! callers swap the original for the sanitized copy. This makes the
//! `no_mutation_in_egress_gate` startup contract directly checkable.

use async_trait::async_trait;

/// Direction the payload is about to flow when `check` is invoked.
///
/// Marked `#[non_exhaustive]` so future kinds (e.g. background telemetry)
/// can be added without breaking external `match` arms.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EgressKind {
    /// Data is about to be sent to the LLM provider over HTTP.
    ///
    /// The canonical scan point for prompts, system messages, and
    /// attachment-extracted text (closes #92).
    LlmRequest,

    /// Data is about to be passed as tool arguments to a tool runtime
    /// (bash, filesystem, MCP server).
    ///
    /// The `tool` field carries the registered tool name so kind-specific
    /// gates (e.g. `BashValidationGate`) can fast-path non-matching tools.
    /// Payload is the JSON-serialised arguments object.
    ToolExecution { tool: String },

    /// Data is about to be rendered to a user-facing surface (Tauri UI,
    /// CLI TTY, SSE stream).
    UserDisplay,

    /// Data is about to be persisted (admin-backend audit, local DB,
    /// conversation tracker).
    Persistence,
}

/// Suggestion surfaced alongside [`EgressDecision::Ask`] so the UX can
/// render actionable buttons ("Always allow `git status`").
///
/// Mirrors the previous `dasclaw_core::RuleSuggestion` API; moved here so
/// `Ask` decisions remain self-contained in `dasclaw_governance` without
/// depending on the agent crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSuggestion {
    /// Human-readable label shown on the button.
    pub label: String,
    /// Rule pattern that would be inserted (e.g. `Bash(git status:allow)`).
    pub rule_pattern: String,
    /// Action implied when the user selects this suggestion.
    pub action: RuleAction,
}

/// Action implied by a [`RuleSuggestion`] when the user selects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuleAction {
    /// Always allow matching invocations.
    Allow,
    /// Always deny matching invocations.
    Deny,
    /// Continue asking on matching invocations.
    Ask,
}

/// Sanitization statistics attached to [`EgressDecision::Redact`] /
/// [`EgressDecision::Block`].
///
/// Empty (`Default`) when the gate does not measure or when no
/// sanitization actually fired (e.g. fail-closed `Block` due to an
/// internal error).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactionStats {
    /// Number of secret-like tokens removed from the payload.
    pub secrets_redacted: usize,
    /// Number of PII tokens removed from the payload.
    pub pii_redacted: usize,
    /// Net bytes changed between input and output (positive when output
    /// shrank). Useful for audit dashboards.
    pub bytes_changed: usize,
}

/// Decision returned by [`EgressGate::check`].
///
/// `#[non_exhaustive]` so future variants do not break external callers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EgressDecision {
    /// Allow the payload to flow downstream unchanged.
    Allow,
    /// Allow the payload but substitute `sanitized` for the original.
    ///
    /// Callers MUST swap the payload they hold before continuing.
    Redact {
        sanitized: String,
        stats: RedactionStats,
    },
    /// Refuse the egress. `reason` is safe to surface to the user.
    Block {
        reason: String,
        stats: RedactionStats,
    },
    /// Request user confirmation. The runtime SHOULD render `suggestions`
    /// as actionable buttons; in headless contexts this MUST be treated
    /// as [`EgressDecision::Block`] (Fail-Safe).
    Ask {
        reason: String,
        suggestions: Vec<RuleSuggestion>,
    },
    /// This gate abstains. In a [`CompositeEgressGate`] chain the next
    /// gate is consulted; in a single-gate context callers MUST treat
    /// this as [`EgressDecision::Allow`].
    Passthrough,
}

/// Layer B trait seam: gate every data egress out of the agent's trust
/// boundary.
///
/// See module-level docs for the four egress kinds and the fail-closed
/// contract.
#[async_trait]
pub trait EgressGate: Send + Sync {
    /// Inspect `payload` heading toward `kind` and return a decision.
    ///
    /// Implementations MUST convert internal errors into
    /// [`EgressDecision::Block`] rather than panicking.
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision;
}

/// No-op gate: every egress is allowed unchanged.
///
/// Useful as the default `HookBundle.egress` in unit tests that do not
/// exercise any safety path.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEgressGate;

#[async_trait]
impl EgressGate for NoopEgressGate {
    async fn check(&self, _kind: &EgressKind, _payload: &str) -> EgressDecision {
        EgressDecision::Allow
    }
}

/// Stable identifier for a gate slot in a [`CompositeEgressGate`] chain.
///
/// Used in tracing logs and audit records to attribute decisions to a
/// specific gate.
pub type GateId = &'static str;

/// Sequential composition of [`EgressGate`] implementations
/// (replaces the previous `CompositeSafetyHook` per ADR-148).
///
/// # Short-circuit semantics (mirrors ADR-147 §2.2)
///
/// For every kind, gates run in insertion order:
///
/// - `Allow` / `Redact` → record latest concrete decision, continue.
///   When a `Redact` fires, the *sanitized payload* is threaded into the
///   next gate so subsequent gates see the rewritten text.
/// - `Passthrough` → continue without recording a concrete decision.
/// - `Block` / `Ask` → short-circuit, return immediately.
///
/// At end of chain: if any gate recorded `Redact`, the final Redact is
/// returned. Otherwise `Allow` is returned (covers both the "all
/// Passthrough" and "at least one Allow" cases).
pub struct CompositeEgressGate {
    gates: Vec<(GateId, std::sync::Arc<dyn EgressGate>)>,
}

impl CompositeEgressGate {
    /// Create a new builder.
    #[must_use]
    pub fn builder() -> CompositeEgressGateBuilder {
        CompositeEgressGateBuilder { gates: Vec::new() }
    }

    /// Number of gates currently composed (for diagnostics / tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.gates.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.gates.is_empty()
    }

    /// Returns the ordered list of [`GateId`]s in the chain.
    pub fn gate_ids(&self) -> Vec<GateId> {
        self.gates.iter().map(|(id, _)| *id).collect()
    }
}

/// Builder for [`CompositeEgressGate`].
pub struct CompositeEgressGateBuilder {
    gates: Vec<(GateId, std::sync::Arc<dyn EgressGate>)>,
}

impl CompositeEgressGateBuilder {
    /// Append a gate to the chain. `id` MUST be a stable identifier
    /// (used in tracing / audit). Duplicate ids are allowed but
    /// discouraged.
    #[must_use]
    pub fn add(mut self, id: GateId, gate: std::sync::Arc<dyn EgressGate>) -> Self {
        self.gates.push((id, gate));
        self
    }

    /// Finalise the builder.
    #[must_use]
    pub fn build(self) -> CompositeEgressGate {
        CompositeEgressGate { gates: self.gates }
    }
}

#[async_trait]
impl EgressGate for CompositeEgressGate {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        // Track the running payload so gates see prior Redact output.
        // `Cow` avoids an allocation for the common "no redact" path.
        let mut current: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(payload);
        // Latest concrete (non-Passthrough) decision. End-of-chain rule:
        // if any gate Redacted, return that final Redact; otherwise Allow.
        let mut last_redact: Option<EgressDecision> = None;

        for (id, gate) in &self.gates {
            let decision = gate.check(kind, &current).await;
            match decision {
                EgressDecision::Allow => {
                    tracing::trace!(gate_id = id, ?kind, "egress: Allow, continue");
                }
                EgressDecision::Redact { sanitized, stats } => {
                    tracing::trace!(
                        gate_id = id,
                        ?kind,
                        "egress: Redact, continue with sanitized"
                    );
                    current = std::borrow::Cow::Owned(sanitized.clone());
                    last_redact = Some(EgressDecision::Redact { sanitized, stats });
                }
                EgressDecision::Passthrough => {
                    tracing::trace!(gate_id = id, ?kind, "egress: Passthrough");
                }
                d @ (EgressDecision::Block { .. } | EgressDecision::Ask { .. }) => {
                    tracing::debug!(gate_id = id, ?kind, decision = ?d, "egress: short-circuit");
                    return d;
                }
            }
        }

        last_redact.unwrap_or(EgressDecision::Allow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Helper gate that returns a fixed decision and remembers the payload
    /// it last observed (so chained tests can assert payload threading).
    struct FixedGate {
        decision: EgressDecision,
        last_seen: std::sync::Mutex<Option<String>>,
    }

    impl FixedGate {
        fn new(decision: EgressDecision) -> Self {
            Self {
                decision,
                last_seen: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl EgressGate for FixedGate {
        async fn check(&self, _kind: &EgressKind, payload: &str) -> EgressDecision {
            *self.last_seen.lock().unwrap() = Some(payload.to_string());
            self.decision.clone()
        }
    }

    #[tokio::test]
    async fn noop_gate_allows_every_kind() {
        let gate = NoopEgressGate;
        for kind in [
            EgressKind::LlmRequest,
            EgressKind::ToolExecution {
                tool: "bash".into(),
            },
            EgressKind::UserDisplay,
            EgressKind::Persistence,
        ] {
            assert_eq!(gate.check(&kind, "x").await, EgressDecision::Allow);
        }
    }

    #[tokio::test]
    async fn composite_returns_allow_on_empty_chain() {
        let gate = CompositeEgressGate::builder().build();
        assert!(gate.is_empty());
        assert_eq!(
            gate.check(&EgressKind::LlmRequest, "hello").await,
            EgressDecision::Allow
        );
    }

    #[tokio::test]
    async fn composite_short_circuits_on_block() {
        let first = Arc::new(FixedGate::new(EgressDecision::Block {
            reason: "bad".into(),
            stats: RedactionStats::default(),
        }));
        let second = Arc::new(FixedGate::new(EgressDecision::Allow));
        let gate = CompositeEgressGate::builder()
            .add("first", first.clone())
            .add("second", second.clone())
            .build();
        let d = gate.check(&EgressKind::LlmRequest, "payload").await;
        assert!(matches!(d, EgressDecision::Block { .. }));
        assert!(
            second.last_seen.lock().unwrap().is_none(),
            "second gate must not be invoked after Block short-circuit"
        );
    }

    #[tokio::test]
    async fn composite_threads_redact_payload_to_next_gate() {
        let first = Arc::new(FixedGate::new(EgressDecision::Redact {
            sanitized: "REDACTED".into(),
            stats: RedactionStats {
                secrets_redacted: 1,
                ..Default::default()
            },
        }));
        let second = Arc::new(FixedGate::new(EgressDecision::Allow));
        let gate = CompositeEgressGate::builder()
            .add("first", first.clone())
            .add("second", second.clone())
            .build();
        let d = gate.check(&EgressKind::LlmRequest, "secret-xyz").await;
        // Final result: the Redact from gate-1 (since gate-2 returned Allow,
        // the chain still surfaces the redaction so the caller swaps).
        match d {
            EgressDecision::Redact { sanitized, stats } => {
                assert_eq!(sanitized, "REDACTED");
                assert_eq!(stats.secrets_redacted, 1);
            }
            other => panic!("expected Redact, got {other:?}"),
        }
        // Gate-2 observed the *sanitized* payload, not the original.
        assert_eq!(
            second.last_seen.lock().unwrap().as_deref(),
            Some("REDACTED"),
            "second gate must see redacted payload"
        );
    }

    #[tokio::test]
    async fn composite_passthrough_only_yields_allow() {
        let g = Arc::new(FixedGate::new(EgressDecision::Passthrough));
        let gate = CompositeEgressGate::builder().add("a", g).build();
        assert_eq!(
            gate.check(&EgressKind::UserDisplay, "x").await,
            EgressDecision::Allow
        );
    }
}
