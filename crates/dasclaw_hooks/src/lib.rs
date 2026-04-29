//! Unified hook engine for the x-claw agent runtime.
//!
//! ## ADR-113 — single front-door for hooks
//!
//! `dasclaw_hooks` is the **single orchestration entry** (`HookRegistry`) for
//! all event-style lifecycle hooks (audit log, declarative regex transforms,
//! outbound webhook notifications, plugin/workspace bundles).
//!
//! Trait seams — `SafetyHook`, `SandboxExecutor`, `SecretProvider`,
//! `ApprovalGate`, `SessionHooks` — are owned by [`x_claw_agent`] (per
//! ADR-001 crate-independence rule) and **reexported here** so callers only
//! need to depend on `dasclaw_hooks`.
//!
//! See [`docs/plans/architecture-refactor/adr-113-hook-engine-unification.md`]
//! for the design rationale, including:
//! - why the two-layer split (event hooks + trait seams) is preserved
//! - the responsibility contract (`no_safety_rule_in_event_hooks`)
//! - the Phase 0 red-line definition (`count_hook_systems() == 1`).

pub mod bundled;
pub mod contract;
pub mod hook;
pub mod registry;

pub use bundled::{
    HookBundleConfig, HookBundleError, HookRegistrationSummary, HookRuleConfig,
    OutboundWebhookConfig, RegexReplacementConfig, register_bundle, register_bundled_hooks,
};
pub use contract::{ContractViolation, count_hook_systems, no_safety_rule_in_event_hooks};
pub use hook::{Hook, HookContext, HookError, HookEvent, HookFailureMode, HookOutcome, HookPoint};
pub use registry::HookRegistry;

// Reexport trait seams from `x_claw_agent` so external callers never have to
// import both crates. The reexports are deliberately type-identity-preserving
// (`pub use`), not newtype wrappers.
pub use x_claw_agent::{
    ApprovalError, ApprovalGate, ApprovalOutcome, ApprovalRequest, AutoApproveGate, DenyAllGate,
    HookBundle, InMemorySecrets, NoopSafetyHook, NoopSandboxExecutor, NoopSessionHooks,
    SafetyDecision, SafetyError, SafetyHook, SandboxError, SandboxExecOutput, SandboxExecRequest,
    SandboxExecutor, SecretError, SecretProvider, SecretString, SessionHooks,
};

/// Bridge [`HookRegistry`] into the `x_claw_agent::SessionHooks` trait so
/// `SessionManager` (in the runtime crate) can fire `OnSessionStart` /
/// `OnSessionEnd` events without depending on a concrete hook engine.
///
/// Errors from hook execution are logged and swallowed — session lifecycle
/// must never be blocked by hook failures (fire-and-forget contract).
///
/// Lives in `dasclaw_hooks` (rather than the runtime or ironclaw) because
/// `HookRegistry` is defined here; placing the impl elsewhere would violate
/// Rust's orphan rule.
#[async_trait::async_trait]
impl x_claw_agent::SessionHooks for HookRegistry {
    async fn on_session_start(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionStart {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionStart hook error: {}", e);
        }
    }

    async fn on_session_end(&self, user_id: &str, session_id: &str) {
        let event = HookEvent::SessionEnd {
            user_id: user_id.to_string(),
            session_id: session_id.to_string(),
        };
        if let Err(e) = self.run(&event).await {
            tracing::warn!("OnSessionEnd hook error: {}", e);
        }
    }
}

#[cfg(test)]
mod acceptance_tests {
    //! ADR-113 §6.1 acceptance tests for P0-3 PR #46.

    use super::*;

    /// `req_p03_pr1_count_hook_systems_is_one` — Phase 0 red-line: exactly
    /// one orchestration entry exists for event-style hooks.
    #[test]
    fn req_p03_pr1_count_hook_systems_is_one() {
        assert_eq!(count_hook_systems(), 1);
    }

    /// `req_p03_pr1_reexport_trait_seams_identity` — the trait seams reexported
    /// from `x_claw_agent` must be the *same* types (no newtype wrappers), so
    /// implementors of `x_claw_agent::SafetyHook` are accepted wherever
    /// `dasclaw_hooks::SafetyHook` is expected.
    #[test]
    fn req_p03_pr1_reexport_trait_seams_identity() {
        fn _assert_same<T: ?Sized>() {}
        // The cast below only type-checks when the two paths resolve to the
        // identical trait object type.
        let _: fn(&dyn x_claw_agent::SafetyHook) -> &dyn SafetyHook = |x| x;
        let _: fn(&dyn x_claw_agent::ApprovalGate) -> &dyn ApprovalGate = |x| x;
        let _: fn(&dyn x_claw_agent::SandboxExecutor) -> &dyn SandboxExecutor = |x| x;
        let _: fn(&dyn x_claw_agent::SecretProvider) -> &dyn SecretProvider = |x| x;
        let _: fn(&dyn x_claw_agent::SessionHooks) -> &dyn SessionHooks = |x| x;
    }

    /// `req_p03_pr1_registry_implements_session_hooks` — `HookRegistry` is
    /// usable as `&dyn SessionHooks` (orphan-rule bridge stays inside this
    /// crate).
    #[test]
    fn req_p03_pr1_registry_implements_session_hooks() {
        let registry = HookRegistry::new();
        let _: &dyn x_claw_agent::SessionHooks = &registry;
    }
}
