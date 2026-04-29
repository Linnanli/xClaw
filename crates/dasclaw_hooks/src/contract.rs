//! Phase 0 red-line contracts for the unified hook engine (ADR-113 §5).
//!
//! Two checks live here:
//!
//! 1. [`count_hook_systems`] — returns the number of hook orchestration
//!    entries. Phase 0 red-line: must be exactly `1` (this crate).
//! 2. [`no_safety_rule_in_event_hooks`] — enforces the responsibility
//!    contract from ADR-113 §2.3: declarative bundle rules registered on the
//!    [`HookRegistry`](crate::HookRegistry) must NOT carry safety/redaction/
//!    secret-blocking semantics. Those belong on the trait seam
//!    [`SafetyHook`](crate::SafetyHook) which returns structured
//!    [`SafetyDecision`](crate::SafetyDecision) values.
//!
//! Both checks are intended to be invoked by `bootstrap_hooks` at startup.
//! Violation handling is left to the caller (panic on startup is the
//! recommended path — fail loud, not silent).

use crate::HookRegistry;

/// Phase 0 red-line: exactly one hook orchestration entry.
///
/// `dasclaw_hooks::HookRegistry` is the single front-door. Trait seams
/// (`SafetyHook` etc.) are reexported from `x_claw_agent` and do **not**
/// count as separate "systems" — see ADR-113 §2.2 for the term clarification.
#[must_use]
pub const fn count_hook_systems() -> usize {
    1
}

/// Reasons a declarative bundle rule violates the safety/event-hook split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolation {
    /// A rule's name contains a banned keyword (`secret`, `redact`,
    /// `safety`) — these responsibilities live on `SafetyHook`.
    NameSuggestsSafetyResponsibility { hook_name: String, keyword: String },
}

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NameSuggestsSafetyResponsibility { hook_name, keyword } => write!(
                f,
                "declarative event-hook rule `{hook_name}` carries safety \
                 responsibility (keyword `{keyword}`); use the SafetyHook \
                 trait seam instead — see ADR-113 §2.3"
            ),
        }
    }
}

impl std::error::Error for ContractViolation {}

const BANNED_KEYWORDS: &[&str] = &["secret", "redact", "safety"];

/// Inspect every hook currently registered on `engine` and return any
/// violation of the ADR-113 §2.3 responsibility contract.
///
/// The check is best-effort and uses hook names as the signal — declarative
/// rules carry stable names supplied by the bundle author or plugin source,
/// so a name like `redact-api-keys` is the canonical leakage indicator.
///
/// Returns an empty `Vec` when no violations are found.
pub async fn no_safety_rule_in_event_hooks(engine: &HookRegistry) -> Vec<ContractViolation> {
    let names = engine.list().await;
    let mut violations = Vec::new();
    for name in names {
        let lower = name.to_lowercase();
        for kw in BANNED_KEYWORDS {
            if lower.contains(kw) {
                violations.push(ContractViolation::NameSuggestsSafetyResponsibility {
                    hook_name: name.clone(),
                    keyword: (*kw).to_string(),
                });
                break;
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::{Hook, HookContext, HookError, HookEvent, HookOutcome, HookPoint};
    use async_trait::async_trait;
    use std::sync::Arc;

    #[test]
    fn count_hook_systems_is_one() {
        assert_eq!(count_hook_systems(), 1);
    }

    struct StaticHook {
        name: &'static str,
        points: Vec<HookPoint>,
    }

    #[async_trait]
    impl Hook for StaticHook {
        fn name(&self) -> &str {
            self.name
        }
        fn hook_points(&self) -> &[HookPoint] {
            &self.points
        }
        async fn execute(
            &self,
            _event: &HookEvent,
            _ctx: &HookContext,
        ) -> Result<HookOutcome, HookError> {
            Ok(HookOutcome::ok())
        }
    }

    #[tokio::test]
    async fn req_p03_pr1_no_safety_rule_in_event_hooks_clean_registry() {
        let engine = HookRegistry::new();
        engine
            .register(Arc::new(StaticHook {
                name: "audit-log",
                points: vec![HookPoint::BeforeToolCall],
            }))
            .await;
        let violations = no_safety_rule_in_event_hooks(&engine).await;
        assert!(
            violations.is_empty(),
            "clean registry should have 0 violations, got {violations:?}"
        );
    }

    #[tokio::test]
    async fn req_p03_pr1_no_safety_rule_in_event_hooks_rejects_violation() {
        let engine = HookRegistry::new();
        engine
            .register(Arc::new(StaticHook {
                name: "redact-api-keys",
                points: vec![HookPoint::BeforeOutbound],
            }))
            .await;
        engine
            .register(Arc::new(StaticHook {
                name: "block-secrets",
                points: vec![HookPoint::BeforeInbound],
            }))
            .await;

        let violations = no_safety_rule_in_event_hooks(&engine).await;
        assert_eq!(
            violations.len(),
            2,
            "expected 2 violations, got {violations:?}"
        );

        let messages: Vec<String> = violations.iter().map(|v| v.to_string()).collect();
        assert!(messages.iter().any(|m| m.contains("redact-api-keys")));
        assert!(messages.iter().any(|m| m.contains("block-secrets")));
    }
}
