//! ADR-149 / issue #485 — `ToolVisibilityPolicy` trait surface tests.
//!
//! These tests pin the public contract of the trait + closed-enum types.
//! They run only when the `tool_visibility` cargo feature is enabled.
//!
//! Behavioral tests for concrete policies (e.g. `BlocklistPolicy` wrapping
//! `feature_flags::disabled_tools`) live in `desktop-client/ironclaw/tests/
//! tool_visibility_integration.rs` — those exercise the L2 / L3 wiring and
//! the five `test_security_84_*` attack scenarios.

#![cfg(feature = "tool_visibility")]

use dasclaw_governance::tool_visibility::{
    ActorId, AllowAllPolicy, ApprovalScope, ApprovalSpec, AuditMetadata, DecisionId, Env,
    SharedToolVisibilityPolicy, TenantId, ToolGateContext, ToolGateContextSeed, ToolGateDecision,
    ToolGateLayer, ToolSource, ToolVisibilityPolicy,
};
use std::sync::Arc;

// ---- Test fixtures ---------------------------------------------------------

#[derive(Debug)]
struct DenyAllPolicy {
    reason: String,
}

#[async_trait::async_trait]
impl ToolVisibilityPolicy for DenyAllPolicy {
    fn name(&self) -> &str {
        "test::deny_all"
    }
    async fn check(&self, _ctx: &ToolGateContext<'_>) -> ToolGateDecision {
        ToolGateDecision::deny_args(&self.reason)
    }
    fn always_check_at_executor(&self, _source: ToolSource) -> bool {
        true
    }
}

fn actor() -> ActorId {
    ActorId::new("alice")
}
fn tenant() -> TenantId {
    TenantId::new("acme")
}

fn ctx<'a>(
    tool: &'a str,
    source: ToolSource,
    layer: ToolGateLayer,
    actor: &'a ActorId,
    tenant: &'a TenantId,
) -> ToolGateContext<'a> {
    ToolGateContext {
        tool_name: tool,
        source,
        layer,
        actor,
        tenant,
        args: None,
        env: Env::Interactive,
    }
}

// ---- Decision surface ------------------------------------------------------

/// `req_tool_visibility_84_001_decision_no_default` — fail-closed by
/// construction. `ToolGateDecision` must require an explicit constructor;
/// there is no `Default` impl that could silently produce `Allow`.
#[test]
fn req_tool_visibility_84_001_decision_no_default() {
    // Compile-time: if `ToolGateDecision: Default` existed, the next line
    // would compile. We assert by trait-not-implemented at runtime via
    // `impls!` would require a dep; settle for documenting via match.
    let allow = ToolGateDecision::Allow;
    let hide = ToolGateDecision::hide("policy redacted");
    let deny = ToolGateDecision::deny_args("invalid args");
    let approval = ToolGateDecision::RequireApproval(ApprovalSpec::session("admin tool"));

    // Exhaustive match — adding a variant without updating this test
    // breaks the build, preventing silent expansion that bypasses
    // downstream pattern matches.
    for d in [allow, hide, deny, approval] {
        match d {
            ToolGateDecision::Allow
            | ToolGateDecision::Hide { .. }
            | ToolGateDecision::DenyArgs { .. }
            | ToolGateDecision::RequireApproval(_) => {}
        }
    }
}

/// `req_tool_visibility_84_002_decision_helpers` — `is_visible` and
/// `permits_execution` semantics fixed in trait spec.
#[test]
fn req_tool_visibility_84_002_decision_helpers() {
    assert!(ToolGateDecision::Allow.is_visible());
    assert!(ToolGateDecision::Allow.permits_execution());

    let approval = ToolGateDecision::RequireApproval(ApprovalSpec::one_time("test"));
    assert!(
        approval.is_visible(),
        "approval-required tools remain visible"
    );
    assert!(
        !approval.permits_execution(),
        "approval-required tools must not auto-execute"
    );

    let hide = ToolGateDecision::hide("hidden");
    assert!(!hide.is_visible());
    assert!(!hide.permits_execution());

    let deny = ToolGateDecision::deny_args("nope");
    assert!(!deny.is_visible());
    assert!(!deny.permits_execution());
}

/// `req_tool_visibility_84_003_decision_constructors_carry_reason` — the
/// `hide` / `deny_args` constructors must round-trip the reason verbatim
/// so audit logs are useful.
#[test]
fn req_tool_visibility_84_003_decision_constructors_carry_reason() {
    match ToolGateDecision::hide("blocked by policy X") {
        ToolGateDecision::Hide { reason } => assert_eq!(reason, "blocked by policy X"),
        other => panic!("expected Hide, got {other:?}"),
    }
    match ToolGateDecision::deny_args("path traversal") {
        ToolGateDecision::DenyArgs { reason } => assert_eq!(reason, "path traversal"),
        other => panic!("expected DenyArgs, got {other:?}"),
    }
}

// ---- Source / Layer / Env exhaustiveness -----------------------------------

/// `req_tool_visibility_84_004_source_5_variants` — five tool sources
/// remain distinct & enumerable. Pins ADR-149 §2.6 surface.
#[test]
fn req_tool_visibility_84_004_source_5_variants() {
    let all = [
        ToolSource::BuiltIn,
        ToolSource::Mcp,
        ToolSource::Skill,
        ToolSource::Extension,
        ToolSource::Wasm,
    ];
    let mut set = std::collections::HashSet::new();
    for s in all {
        assert!(set.insert(s), "duplicate variant in ToolSource");
    }
    assert_eq!(set.len(), 5);
}

/// `req_tool_visibility_84_005_layer_3_variants` — three layers, distinct.
#[test]
fn req_tool_visibility_84_005_layer_3_variants() {
    let all = [
        ToolGateLayer::PromptCatalog,
        ToolGateLayer::LlmDefinitions,
        ToolGateLayer::ExecutorPreflight,
    ];
    let mut set = std::collections::HashSet::new();
    for l in all {
        assert!(set.insert(l), "duplicate variant in ToolGateLayer");
    }
    assert_eq!(set.len(), 3);
}

/// `req_tool_visibility_84_006_env_3_variants` — three execution
/// environments, distinct.
#[test]
fn req_tool_visibility_84_006_env_3_variants() {
    let all = [Env::Interactive, Env::Autonomous, Env::Container];
    let mut set = std::collections::HashSet::new();
    for e in all {
        assert!(set.insert(e), "duplicate variant in Env");
    }
    assert_eq!(set.len(), 3);
}

// ---- ApprovalSpec ----------------------------------------------------------

/// `req_tool_visibility_84_007_approval_one_time` — `one_time` shape
/// fixed: no "always" upgrade, scope is `OneTime`.
#[test]
fn req_tool_visibility_84_007_approval_one_time() {
    let a = ApprovalSpec::one_time("destructive op");
    assert!(!a.allow_always);
    assert_eq!(a.scope, ApprovalScope::OneTime);
    assert_eq!(a.reason, "destructive op");
}

/// `req_tool_visibility_84_008_approval_session` — session approval is
/// allow_always=true, scope=Session.
#[test]
fn req_tool_visibility_84_008_approval_session() {
    let a = ApprovalSpec::session("network access");
    assert!(a.allow_always);
    assert_eq!(a.scope, ApprovalScope::Session);
    assert_eq!(a.reason, "network access");
}

// ---- Audit metadata serde round-trip --------------------------------------

/// `req_tool_visibility_84_009_audit_serde_round_trip` — `AuditMetadata`
/// serializes / deserializes via serde_json (telemetry pipeline contract).
#[test]
fn req_tool_visibility_84_009_audit_serde_round_trip() {
    let meta = AuditMetadata {
        policy_name: "BlocklistPolicy".into(),
        layer: ToolGateLayer::LlmDefinitions,
        decision_id: DecisionId::new("01HXXX..."),
        source: ToolSource::BuiltIn,
        actor: actor(),
        tenant: tenant(),
    };
    let json = serde_json::to_string(&meta).expect("serialize");
    let back: AuditMetadata = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(meta, back);
}

/// `req_tool_visibility_84_010_decision_serde_round_trip` — decisions
/// serialize via the `kind`-tagged form (telemetry contract).
#[test]
fn req_tool_visibility_84_010_decision_serde_round_trip() {
    let cases = [
        ToolGateDecision::Allow,
        ToolGateDecision::hide("h"),
        ToolGateDecision::deny_args("d"),
        ToolGateDecision::RequireApproval(ApprovalSpec::session("approve me")),
    ];
    for d in cases {
        let json = serde_json::to_string(&d).expect("serialize");
        let back: ToolGateDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }
}

// ---- Trait object & async surface ------------------------------------------

/// `req_tool_visibility_84_011_trait_object_safe` — `ToolVisibilityPolicy`
/// is dyn-safe and goes into `Arc<dyn …>`. Required by ADR-149 §2.5
/// (Always-Has-Policy invariant: hosts hold the policy as Arc).
#[tokio::test]
async fn req_tool_visibility_84_011_trait_object_safe() {
    let p: SharedToolVisibilityPolicy = Arc::new(AllowAllPolicy);
    assert_eq!(p.name(), "dasclaw_governance::AllowAllPolicy");

    let actor = actor();
    let tenant = tenant();
    let c = ctx(
        "shell",
        ToolSource::BuiltIn,
        ToolGateLayer::LlmDefinitions,
        &actor,
        &tenant,
    );
    let d = p.check(&c).await;
    assert!(matches!(d, ToolGateDecision::Allow));
}

/// `req_tool_visibility_84_012_always_check_at_executor` — default impl
/// returns false; override returns true. Mirrors claude-code
/// `requireCanUseTool` opt-in.
#[tokio::test]
async fn req_tool_visibility_84_012_always_check_at_executor() {
    let lax: SharedToolVisibilityPolicy = Arc::new(AllowAllPolicy);
    assert!(!lax.always_check_at_executor(ToolSource::BuiltIn));
    assert!(!lax.always_check_at_executor(ToolSource::Mcp));

    let strict: SharedToolVisibilityPolicy = Arc::new(DenyAllPolicy {
        reason: "test".into(),
    });
    assert!(strict.always_check_at_executor(ToolSource::BuiltIn));
}

// ---- Fail-closed audit -----------------------------------------------------

/// `req_tool_visibility_84_013_deny_carries_reason_through_async` —
/// async `check` returns a deny decision with the original reason text,
/// so the audit pipeline never loses the "why".
#[tokio::test]
async fn req_tool_visibility_84_013_deny_carries_reason_through_async() {
    let p: SharedToolVisibilityPolicy = Arc::new(DenyAllPolicy {
        reason: "kill-switch engaged".into(),
    });
    let actor = actor();
    let tenant = tenant();
    let c = ctx(
        "shell",
        ToolSource::BuiltIn,
        ToolGateLayer::ExecutorPreflight,
        &actor,
        &tenant,
    );
    match p.check(&c).await {
        ToolGateDecision::DenyArgs { reason } => {
            assert_eq!(reason, "kill-switch engaged");
        }
        other => panic!("expected DenyArgs, got {other:?}"),
    }
}

// ---- Identity newtype contract ---------------------------------------------

/// `req_tool_visibility_84_014_identity_newtypes_round_trip` — `ActorId`
/// / `TenantId` preserve their string payload (audit log contract).
/// TODO(identity-W2): once `dasclaw_identity::ActorRef` is real, this
/// test moves to the identity crate and the newtypes are deleted.
#[test]
fn req_tool_visibility_84_014_identity_newtypes_round_trip() {
    let a = ActorId::new("user-42");
    assert_eq!(a.as_str(), "user-42");
    assert_eq!(a.to_string(), "user-42");
    assert_eq!(a, ActorId::new("user-42"));

    let t = TenantId::new("tenant-7");
    assert_eq!(t.as_str(), "tenant-7");
    assert_eq!(t.to_string(), "tenant-7");
}

// ---- AllowAllPolicy (production placeholder) ------------------------------

/// `req_tool_visibility_84_015_allow_all_policy_default` — the canonical
/// "default permissive" policy returns `Allow` regardless of input and
/// carries a stable `name()` for audit-log identification. This is the
/// Always-Has-Policy invariant's escape hatch: callers without a real
/// policy yet pass `Arc::new(AllowAllPolicy)` rather than `Option::None`.
#[tokio::test]
async fn req_tool_visibility_84_015_allow_all_policy_default() {
    let p: SharedToolVisibilityPolicy = Arc::new(AllowAllPolicy);
    assert_eq!(p.name(), "dasclaw_governance::AllowAllPolicy");
    assert!(!p.always_check_at_executor(ToolSource::BuiltIn));

    let actor = actor();
    let tenant = tenant();
    for layer in [
        ToolGateLayer::PromptCatalog,
        ToolGateLayer::LlmDefinitions,
        ToolGateLayer::ExecutorPreflight,
    ] {
        for source in [
            ToolSource::BuiltIn,
            ToolSource::Mcp,
            ToolSource::Skill,
            ToolSource::Extension,
            ToolSource::Wasm,
        ] {
            let c = ctx("any", source, layer, &actor, &tenant);
            match p.check(&c).await {
                ToolGateDecision::Allow => {}
                other => panic!("AllowAllPolicy must return Allow, got {other:?}"),
            }
        }
    }
}

// ---- ToolGateContextSeed --------------------------------------------------

/// `req_tool_visibility_84_016_context_seed_system_default` — the `system`
/// constructor produces the literal `"system"` / `"default"` IDs that
/// `grep` can locate for the W2 identity migration. Round-trips through
/// `new(...)` and preserves `env`.
#[test]
fn req_tool_visibility_84_016_context_seed_system_default() {
    let s = ToolGateContextSeed::system(Env::Interactive);
    assert_eq!(s.actor.as_str(), "system");
    assert_eq!(s.tenant.as_str(), "default");
    assert!(matches!(s.env, Env::Interactive));

    let s2 = ToolGateContextSeed::new(
        ActorId::new("user-1"),
        TenantId::new("tenant-a"),
        Env::Container,
    );
    assert_eq!(s2.actor.as_str(), "user-1");
    assert_eq!(s2.tenant.as_str(), "tenant-a");
    assert!(matches!(s2.env, Env::Container));
}
