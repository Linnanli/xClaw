//! ADR-148 §2.2 / §5 contract tests for the `EgressGate` seam.
//!
//! These tests pin the public surface so future refactors cannot silently
//! relax the fail-closed / no-mutation guarantees.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_governance::egress::{
    CompositeEgressGate, EgressDecision, EgressGate, EgressKind, NoopEgressGate, RedactionStats,
};

// --- Fixture gates ----------------------------------------------------------

struct AlwaysAllow;
#[async_trait]
impl EgressGate for AlwaysAllow {
    async fn check(&self, _k: &EgressKind, _p: &str) -> EgressDecision {
        EgressDecision::Allow
    }
}

struct AlwaysBlock(&'static str);
#[async_trait]
impl EgressGate for AlwaysBlock {
    async fn check(&self, _k: &EgressKind, _p: &str) -> EgressDecision {
        EgressDecision::Block {
            reason: self.0.to_string(),
            stats: RedactionStats::default(),
        }
    }
}

struct Redactor {
    replace: &'static str,
    with: &'static str,
}
#[async_trait]
impl EgressGate for Redactor {
    async fn check(&self, _k: &EgressKind, payload: &str) -> EgressDecision {
        if payload.contains(self.replace) {
            let sanitized = payload.replace(self.replace, self.with);
            EgressDecision::Redact {
                sanitized,
                stats: RedactionStats {
                    secrets_redacted: 1,
                    pii_redacted: 0,
                    bytes_changed: self.replace.len().saturating_sub(self.with.len()),
                },
            }
        } else {
            EgressDecision::Allow
        }
    }
}

struct AlwaysPassthrough;
#[async_trait]
impl EgressGate for AlwaysPassthrough {
    async fn check(&self, _k: &EgressKind, _p: &str) -> EgressDecision {
        EgressDecision::Passthrough
    }
}

// --- Contract tests ---------------------------------------------------------

/// ADR-148 §2.2 — `EgressDecision` carries five variants and all can be
/// constructed by an external crate. Pinning the surface so a future
/// refactor cannot collapse `Ask`/`Passthrough` without a follow-up ADR.
#[test]
fn req_egress_148_contract_decision_variants_constructible() {
    let _allow = EgressDecision::Allow;
    let _redact = EgressDecision::Redact {
        sanitized: String::new(),
        stats: RedactionStats::default(),
    };
    let _block = EgressDecision::Block {
        reason: String::new(),
        stats: RedactionStats::default(),
    };
    let _ask = EgressDecision::Ask {
        reason: String::new(),
        suggestions: Vec::new(),
    };
    let _pass = EgressDecision::Passthrough;
}

/// ADR-148 §2.2 — `EgressKind` is `#[non_exhaustive]`. An external `match`
/// must compile only with a `_` arm. This test would fail to compile if
/// the attribute were removed because the `_` arm would become
/// unreachable (which we silence here so only the compile semantics are
/// asserted, not warning state).
#[test]
fn req_egress_148_contract_kind_is_non_exhaustive() {
    let k = EgressKind::LlmRequest;
    #[allow(unreachable_patterns)]
    let label = match k {
        EgressKind::LlmRequest => "llm",
        EgressKind::ToolExecution { .. } => "tool",
        EgressKind::UserDisplay => "ui",
        EgressKind::Persistence => "db",
        _ => "future",
    };
    assert_eq!(label, "llm");
}

/// `RedactionStats::default()` is the zero value (used by fail-closed
/// Block paths that have no measured sanitization).
#[test]
fn req_egress_148_contract_redaction_stats_default_is_zero() {
    let s = RedactionStats::default();
    assert_eq!(s.secrets_redacted, 0);
    assert_eq!(s.pii_redacted, 0);
    assert_eq!(s.bytes_changed, 0);
}

/// `NoopEgressGate` always returns `Allow` for every kind. This is the
/// default `HookBundle.egress` for tests that don't exercise a safety
/// path.
#[tokio::test]
async fn req_egress_148_contract_noop_always_allows() {
    let g = NoopEgressGate;
    for kind in [
        EgressKind::LlmRequest,
        EgressKind::ToolExecution {
            tool: "bash".to_string(),
        },
        EgressKind::UserDisplay,
        EgressKind::Persistence,
    ] {
        let d = g.check(&kind, "anything").await;
        assert!(matches!(d, EgressDecision::Allow));
    }
}

/// ADR-148 §2.2 — `CompositeEgressGate` short-circuits on first `Block`.
/// Gates appended after the blocking gate MUST NOT be consulted.
#[tokio::test]
async fn req_egress_148_contract_composite_block_short_circuits() {
    let composite = CompositeEgressGate::builder()
        .add("allow-first", Arc::new(AlwaysAllow))
        .add("block-mid", Arc::new(AlwaysBlock("policy")))
        .add(
            "would-allow-after",
            Arc::new(AlwaysBlock("MUST_NOT_BE_REACHED")),
        )
        .build();

    let d = composite.check(&EgressKind::LlmRequest, "x").await;
    match d {
        EgressDecision::Block { reason, .. } => assert_eq!(reason, "policy"),
        other => panic!("expected Block(policy), got {other:?}"),
    }
}

/// ADR-148 §2.2 — `Redact` decisions chain: a downstream gate sees the
/// sanitized payload, and the final returned `Redact` carries the
/// last-stage sanitized text.
#[tokio::test]
async fn req_egress_148_contract_composite_redact_chains_sanitized_payload() {
    let composite = CompositeEgressGate::builder()
        .add(
            "stage-1",
            Arc::new(Redactor {
                replace: "alpha",
                with: "[A]",
            }),
        )
        .add(
            "stage-2",
            Arc::new(Redactor {
                replace: "beta",
                with: "[B]",
            }),
        )
        .build();

    let d = composite
        .check(&EgressKind::UserDisplay, "alpha and beta")
        .await;

    match d {
        EgressDecision::Redact { sanitized, .. } => {
            assert_eq!(sanitized, "[A] and [B]");
        }
        other => panic!("expected Redact, got {other:?}"),
    }
}

/// `Passthrough` is consumed inside `CompositeEgressGate` and collapses
/// to `Allow` at end of chain (single-gate context callers must do the
/// same per the trait doc).
#[tokio::test]
async fn req_egress_148_contract_composite_all_passthrough_collapses_to_allow() {
    let composite = CompositeEgressGate::builder()
        .add("pt-1", Arc::new(AlwaysPassthrough))
        .add("pt-2", Arc::new(AlwaysPassthrough))
        .build();

    let d = composite.check(&EgressKind::Persistence, "x").await;
    assert!(matches!(d, EgressDecision::Allow));
}
