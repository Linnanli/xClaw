//! Shared helper for applying an [`EgressDecision`] to a mutable payload.
//!
//! Every consumer of [`EgressGate::check`](crate::hooks::EgressGate::check)
//! repeats the same five-arm match: Allow / Passthrough → continue, Redact →
//! swap payload, Block / Ask / unknown variant → halt the caller with a
//! Fail-Safe error string.
//!
//! `apply_egress_decision` centralises this pattern so:
//!
//! 1. The agentic loop's `LlmRequest` and `UserDisplay` sites stop carrying
//!    two near-identical 30-line match blocks (PR-A follow-up).
//! 2. Ironclaw's tool-output sanitisation call sites (PR-C R2, ADR-148
//!    epic #483) all halt identically when the gate refuses.
//! 3. The `#[non_exhaustive]` guard on
//!    [`EgressDecision`](crate::hooks::EgressDecision) is enforced in one
//!    place — future variants surface a single Fail-Safe code path instead
//!    of being silently allowed by callers that forgot to match them.

use crate::hooks::EgressDecision;

/// Outcome of applying an [`EgressDecision`] to a payload.
///
/// Returned by [`apply_egress_decision`]. The caller decides how to lift
/// `Halt(reason)` into its native error type (e.g.
/// [`LoopOutcome::Failure`](crate::agentic_loop::LoopOutcome::Failure),
/// `anyhow::Error`, ironclaw's `HostError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EgressApply {
    /// `Allow` / `Passthrough` / `Redact` — payload may have been swapped
    /// in place; caller continues normally.
    Continue,
    /// `Block` / `Ask` / unknown variant — caller must halt with the
    /// contained Fail-Safe reason.
    Halt(String),
}

/// Apply an [`EgressDecision`] to `payload`, returning whether the caller
/// should continue or halt.
///
/// - [`EgressDecision::Allow`] / [`EgressDecision::Passthrough`] →
///   [`EgressApply::Continue`], `payload` untouched.
/// - [`EgressDecision::Redact`] → `payload` is replaced with the sanitised
///   version; [`EgressApply::Continue`].
/// - [`EgressDecision::Block`] → emits `tracing::warn!` and returns
///   [`EgressApply::Halt`] with `"egress gate blocked {kind_label}: {reason}"`.
/// - [`EgressDecision::Ask`] in a headless caller has no UI to render; per
///   ADR-148 §2.2 and ADR-146 §2.5 this is treated as Fail-Safe block.
///   Slice D will replace this with real Ask UX on the desktop client.
/// - Future variants of the `#[non_exhaustive]` enum hit the
///   `#[allow(unreachable_patterns)]` arm and are logged as an error
///   before halting; this is the single Fail-Safe guard for the whole
///   workspace.
///
/// `kind_label` is a short human-readable label for the gate site (e.g.
/// `"LlmRequest"`, `"UserDisplay"`, `"ToolOutput:bash"`). It appears in
/// both the trace event and the halt reason so log lines and failure
/// messages are self-describing.
pub fn apply_egress_decision(
    decision: EgressDecision,
    payload: &mut String,
    kind_label: &str,
) -> EgressApply {
    match decision {
        EgressDecision::Allow | EgressDecision::Passthrough => EgressApply::Continue,
        EgressDecision::Redact { sanitized, .. } => {
            *payload = sanitized;
            EgressApply::Continue
        }
        EgressDecision::Block { reason, .. } => {
            tracing::warn!(kind = kind_label, %reason, "egress gate blocked");
            EgressApply::Halt(format!("egress gate blocked {kind_label}: {reason}"))
        }
        EgressDecision::Ask { reason, .. } => {
            tracing::warn!(
                kind = kind_label,
                %reason,
                "egress gate returned Ask in headless context; Fail-Safe block (slice D will wire UI)"
            );
            EgressApply::Halt(format!(
                "egress gate requires user confirmation for {kind_label} (no UI available): {reason}"
            ))
        }
        // The `EgressDecision` enum is `#[non_exhaustive]`. Any future
        // variant added in the defining crate must be addressed
        // explicitly here; failing closed is Fail-Safe.
        // `#[allow(unreachable_patterns)]` is required because, within
        // the defining crate, the compiler already sees the enum as
        // exhaustive — out-of-crate callers do not.
        #[allow(unreachable_patterns)]
        other => {
            tracing::error!(
                kind = kind_label,
                decision = ?other,
                "egress gate returned unhandled EgressDecision variant; Fail-Safe block"
            );
            EgressApply::Halt(format!(
                "egress gate returned unsupported decision for {kind_label}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::RedactionStats;

    #[test]
    fn allow_returns_continue_without_touching_payload() {
        let mut payload = String::from("hello");
        let out = apply_egress_decision(EgressDecision::Allow, &mut payload, "LlmRequest");
        assert_eq!(out, EgressApply::Continue);
        assert_eq!(payload, "hello");
    }

    #[test]
    fn passthrough_returns_continue_without_touching_payload() {
        let mut payload = String::from("hello");
        let out = apply_egress_decision(EgressDecision::Passthrough, &mut payload, "UserDisplay");
        assert_eq!(out, EgressApply::Continue);
        assert_eq!(payload, "hello");
    }

    #[test]
    fn redact_swaps_payload_in_place() {
        let mut payload = String::from("secret=abc123");
        let out = apply_egress_decision(
            EgressDecision::Redact {
                sanitized: "secret=[REDACTED]".to_string(),
                stats: RedactionStats::default(),
            },
            &mut payload,
            "UserDisplay",
        );
        assert_eq!(out, EgressApply::Continue);
        assert_eq!(payload, "secret=[REDACTED]");
    }

    #[test]
    fn block_returns_halt_with_label_and_reason() {
        let mut payload = String::from("contraband");
        let out = apply_egress_decision(
            EgressDecision::Block {
                reason: "policy violation".to_string(),
                stats: RedactionStats::default(),
            },
            &mut payload,
            "LlmRequest",
        );
        match out {
            EgressApply::Halt(msg) => {
                assert!(msg.contains("egress gate blocked"));
                assert!(msg.contains("LlmRequest"));
                assert!(msg.contains("policy violation"));
            }
            EgressApply::Continue => panic!("expected Halt"),
        }
        // Block does not mutate the payload — the caller decides whether
        // to discard it or surface a placeholder.
        assert_eq!(payload, "contraband");
    }

    #[test]
    fn ask_in_headless_context_returns_halt() {
        let mut payload = String::from("needs approval");
        let out = apply_egress_decision(
            EgressDecision::Ask {
                reason: "high risk".to_string(),
                suggestions: Vec::new(),
            },
            &mut payload,
            "UserDisplay",
        );
        match out {
            EgressApply::Halt(msg) => {
                assert!(msg.contains("requires user confirmation"));
                assert!(msg.contains("UserDisplay"));
                assert!(msg.contains("high risk"));
            }
            EgressApply::Continue => panic!("expected Halt"),
        }
    }

    #[test]
    fn redact_with_empty_sanitized_still_swaps() {
        // Edge case: Redact with empty content is still a valid decision
        // (e.g. a leak detector that nukes the whole payload). The helper
        // must not silently treat empty sanitized as Allow.
        let mut payload = String::from("all secret");
        let out = apply_egress_decision(
            EgressDecision::Redact {
                sanitized: String::new(),
                stats: RedactionStats::default(),
            },
            &mut payload,
            "Persistence",
        );
        assert_eq!(out, EgressApply::Continue);
        assert_eq!(payload, "");
    }
}
