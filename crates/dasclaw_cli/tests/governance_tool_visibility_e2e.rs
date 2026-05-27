//! PR2 GUI-only blind-spot: `dasclaw_governance::tool_visibility` from
//! a non-GUI consumer (ADR-149 / ADR-153 §1).
//!
//! The trait + closed-enum surface is already covered in-crate by
//! `crates/dasclaw_governance/tests/tool_visibility_test.rs`. What that
//! file does **not** cover is the cross-crate wiring path: the CLI is
//! the only non-GUI top-level binary, and downstream consumers wire the
//! policy through `Arc<dyn ToolVisibilityPolicy>` from an external
//! crate's address space. If the trait's object-safety, the
//! `Send + Sync + Debug` bound, or the `tool_visibility` feature gate
//! ever drifts, the failure surfaces here (and not in the in-crate
//! test, which links the trait from the same compilation unit).
//!
//! Scope:
//! 1. Drive `AllowAllPolicy` through both gate layers
//!    (`LlmDefinitions` / `ExecutorPreflight`) — pin the trait-object
//!    path from a downstream consumer.
//! 2. Define a local `DenyAllPolicy` fixture and prove
//!    `permits_execution() == false` on its decision.
//! 3. `RequireApproval` decision is visible but not executable.
//! 4. `AuditMetadata` round-trip (Debug-stable + field shape).
//!
//! Note: `dasclaw_cli` itself has no feature flag here; the
//! `tool_visibility` feature is forwarded *into* the `dasclaw_governance`
//! dev-dep unconditionally in `Cargo.toml`, so this test compiles whenever
//! the CLI test target compiles.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_governance::tool_visibility::{
    ActorId, AllowAllPolicy, ApprovalScope, ApprovalSpec, AuditMetadata, DecisionId, Env,
    SharedToolVisibilityPolicy, TenantId, ToolGateContext, ToolGateDecision, ToolGateLayer,
    ToolSource, ToolVisibilityPolicy,
};

#[derive(Debug)]
struct DenyAllForCli {
    reason: String,
}

#[async_trait]
impl ToolVisibilityPolicy for DenyAllForCli {
    fn name(&self) -> &str {
        "dasclaw_cli::test::deny_all"
    }
    async fn check(&self, _ctx: &ToolGateContext<'_>) -> ToolGateDecision {
        ToolGateDecision::deny_args(&self.reason)
    }
    fn always_check_at_executor(&self, _source: ToolSource) -> bool {
        true
    }
}

fn make_ctx<'a>(
    tool: &'a str,
    layer: ToolGateLayer,
    actor: &'a ActorId,
    tenant: &'a TenantId,
) -> ToolGateContext<'a> {
    ToolGateContext {
        tool_name: tool,
        source: ToolSource::BuiltIn,
        layer,
        actor,
        tenant,
        args: None,
        env: Env::Autonomous,
    }
}

/// `req_dasclaw_cli_governance_pr2_allow_all_through_shared_arc` —
/// drive `AllowAllPolicy` via `SharedToolVisibilityPolicy`
/// (`Arc<dyn ToolVisibilityPolicy>`) from a downstream crate. Catches
/// any drift in trait object-safety or the `Send + Sync + Debug` bound.
#[tokio::test]
async fn req_dasclaw_cli_governance_pr2_allow_all_through_shared_arc() {
    let policy: SharedToolVisibilityPolicy = Arc::new(AllowAllPolicy);
    let actor = ActorId::new("alice");
    let tenant = TenantId::new("acme");

    for layer in [
        ToolGateLayer::LlmDefinitions,
        ToolGateLayer::ExecutorPreflight,
    ] {
        let ctx = make_ctx("shell", layer, &actor, &tenant);
        let decision = policy.check(&ctx).await;
        assert!(
            matches!(decision, ToolGateDecision::Allow),
            "AllowAllPolicy must return Allow at layer {layer:?}, got {decision:?}"
        );
        assert!(decision.is_visible());
        assert!(decision.permits_execution());
    }
    assert!(
        !policy.always_check_at_executor(ToolSource::BuiltIn),
        "AllowAllPolicy must not opt into re-check at executor by default"
    );
}

/// `req_dasclaw_cli_governance_pr2_deny_policy_blocks_execution` — a
/// downstream `DenyAll` policy returns `DenyArgs`. Per
/// `ToolGateDecision::is_visible()` the helper conflates `Hide` and
/// `DenyArgs` as "not visible" (only `Allow` and `RequireApproval`
/// remain visible); the prompt-cache nuance from the doc comment lives
/// at the policy layer, not on the decision helper. Either way, both
/// `is_visible()` and `permits_execution()` MUST be `false` here.
#[tokio::test]
async fn req_dasclaw_cli_governance_pr2_deny_policy_blocks_execution() {
    let policy: SharedToolVisibilityPolicy = Arc::new(DenyAllForCli {
        reason: "blocked by CLI test policy".into(),
    });
    let actor = ActorId::new("bob");
    let tenant = TenantId::new("acme");
    let ctx = make_ctx("shell", ToolGateLayer::ExecutorPreflight, &actor, &tenant);

    let decision = policy.check(&ctx).await;
    match &decision {
        ToolGateDecision::DenyArgs { reason } => {
            assert!(reason.contains("blocked"), "reason text must reach caller");
        }
        other => panic!("expected DenyArgs, got {other:?}"),
    }
    assert!(
        !decision.is_visible(),
        "DenyArgs is not visible per is_visible() — only Allow / RequireApproval are"
    );
    assert!(
        !decision.permits_execution(),
        "DenyArgs MUST NOT permit execution"
    );
    assert!(
        policy.always_check_at_executor(ToolSource::BuiltIn),
        "DenyAllForCli opts into executor-side re-check"
    );
}

/// `req_dasclaw_cli_governance_pr2_hide_decision_invisible_and_blocks`
/// — verify the `hide` helper and the L2 invisibility contract: hidden
/// tools must NOT be visible AND must not be executable. A regression
/// where `hide()` accidentally still permits execution would break the
/// L2/L3 promise the LLM-definitions layer relies on.
#[test]
fn req_dasclaw_cli_governance_pr2_hide_decision_invisible_and_blocks() {
    let d = ToolGateDecision::hide("not in this profile");
    assert!(!d.is_visible(), "hide() must produce an invisible decision");
    assert!(
        !d.permits_execution(),
        "hide() must not permit execution either"
    );

    let d2 = ToolGateDecision::deny_args("bad shape");
    assert!(
        !d2.is_visible(),
        "deny_args() is not visible per is_visible() helper"
    );
    assert!(!d2.permits_execution());
}

/// `req_dasclaw_cli_governance_pr2_require_approval_visible_not_executable`
/// — pin the `RequireApproval` semantics + the `one_time` / `session`
/// constructors that downstream callers use to build the decision.
#[test]
fn req_dasclaw_cli_governance_pr2_require_approval_visible_not_executable() {
    let one = ToolGateDecision::RequireApproval(ApprovalSpec::one_time("dangerous"));
    assert!(one.is_visible(), "approval-needed tools stay visible at L2");
    assert!(
        !one.permits_execution(),
        "approval-needed tools must NOT execute without UI approval"
    );
    match &one {
        ToolGateDecision::RequireApproval(spec) => {
            assert_eq!(spec.scope, ApprovalScope::OneTime);
            assert!(spec.reason.contains("dangerous"));
        }
        other => panic!("expected RequireApproval, got {other:?}"),
    }

    let sess = ApprovalSpec::session("trust for this session");
    assert_eq!(sess.scope, ApprovalScope::Session);
}

/// `req_dasclaw_cli_governance_pr2_audit_metadata_field_shape` —
/// `AuditMetadata` is the bridge type written into audit logs. A
/// downstream consumer must be able to construct it without reaching
/// for private constructors. Pin the field shape + Debug formatting
/// stability (used by `tracing::field::debug` in the GUI).
#[test]
fn req_dasclaw_cli_governance_pr2_audit_metadata_field_shape() {
    let meta = AuditMetadata {
        policy_name: "dasclaw_cli::test::deny_all".into(),
        layer: ToolGateLayer::ExecutorPreflight,
        decision_id: DecisionId::new("dec-1"),
        source: ToolSource::BuiltIn,
        actor: ActorId::new("bob"),
        tenant: TenantId::new("acme"),
    };
    assert_eq!(meta.policy_name, "dasclaw_cli::test::deny_all");
    assert!(matches!(meta.layer, ToolGateLayer::ExecutorPreflight));
    assert_eq!(meta.decision_id.as_str(), "dec-1");
    assert!(matches!(meta.source, ToolSource::BuiltIn));

    let dbg = format!("{meta:?}");
    assert!(
        dbg.contains("dasclaw_cli::test::deny_all"),
        "Debug must surface policy_name (used by tracing); got {dbg}"
    );
}
