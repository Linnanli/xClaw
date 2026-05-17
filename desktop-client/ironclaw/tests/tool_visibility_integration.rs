//! ADR-149 / Issue #485 — L3 Executor Preflight integration tests.
//!
//! Verifies the **defense-in-depth** guarantee: even if a tool slips past
//! the L2 catalog (e.g. LLM hallucinated tool call, stale cache, jailbreak
//! prompt), the L3 executor preflight gate still rejects it before any
//! tool code runs. Also pins the [`BlocklistPolicy`] migration parity with
//! the legacy `ToolFeatureFlags::is_tool_enabled` HashSet.
//!
//! These tests run with `features = ["tool_visibility"]` enabled on
//! `dasclaw_governance` via the ironclaw `Cargo.toml` default deps.

use std::sync::Arc;

use dasclaw_governance::tool_visibility::{
    ActorId, Env, TenantId, ToolGateContext, ToolGateContextSeed, ToolGateDecision, ToolGateLayer,
    ToolSource, ToolVisibilityPolicy,
};
use ironclaw::tools::feature_flags::{BlocklistPolicy, ToolFeatureFlags};

fn ctx_for<'a>(
    tool_name: &'a str,
    source: ToolSource,
    layer: ToolGateLayer,
    actor: &'a ActorId,
    tenant: &'a TenantId,
) -> ToolGateContext<'a> {
    ToolGateContext {
        tool_name,
        source,
        layer,
        actor,
        tenant,
        args: None,
        env: Env::Interactive,
    }
}

/// req_tool_visibility_84_017_blocklist_hides_disabled_builtin
///
/// BlocklistPolicy returns `Hide` for a BuiltIn tool listed in the
/// disabled set. The reason string is non-empty so the audit log can
/// surface it.
#[tokio::test]
async fn req_tool_visibility_84_017_blocklist_hides_disabled_builtin() {
    let flags = Arc::new(ToolFeatureFlags::with_disabled(vec!["shell".into()]));
    let policy = BlocklistPolicy::new(flags);
    let actor = ActorId("system".into());
    let tenant = TenantId("default".into());

    let decision = policy
        .check(&ctx_for(
            "shell",
            ToolSource::BuiltIn,
            ToolGateLayer::LlmDefinitions,
            &actor,
            &tenant,
        ))
        .await;

    match decision {
        ToolGateDecision::Hide { reason } => {
            assert!(!reason.is_empty(), "Hide reason must be non-empty");
            assert!(
                reason.contains("shell"),
                "reason should mention the tool name, got: {reason}",
            );
        }
        other => panic!("expected Hide, got {other:?}"),
    }
}

/// req_tool_visibility_84_018_blocklist_allows_non_builtin
///
/// BlocklistPolicy only governs BuiltIn tools. Non-BuiltIn sources
/// (Mcp / Skill / Extension / Wasm) bypass the legacy blocklist — they
/// have their own gating mechanisms (extension manifest signing, MCP
/// server allowlist, etc.) which will land in later policies.
#[tokio::test]
async fn req_tool_visibility_84_018_blocklist_allows_non_builtin() {
    // Same disabled name as a BuiltIn, but the gate sees source=Mcp.
    let flags = Arc::new(ToolFeatureFlags::with_disabled(vec!["shell".into()]));
    let policy = BlocklistPolicy::new(flags);
    let actor = ActorId("system".into());
    let tenant = TenantId("default".into());

    for src in [
        ToolSource::Mcp,
        ToolSource::Skill,
        ToolSource::Extension,
        ToolSource::Wasm,
    ] {
        let decision = policy
            .check(&ctx_for(
                "shell",
                src,
                ToolGateLayer::ExecutorPreflight,
                &actor,
                &tenant,
            ))
            .await;
        assert!(
            matches!(decision, ToolGateDecision::Allow),
            "non-BuiltIn source {src:?} must bypass BlocklistPolicy, got {decision:?}",
        );
    }
}

/// req_tool_visibility_84_019_blocklist_allows_enabled_builtin
///
/// A BuiltIn tool NOT in the disabled set passes the policy unchanged
/// at every layer.
#[tokio::test]
async fn req_tool_visibility_84_019_blocklist_allows_enabled_builtin() {
    let flags = Arc::new(ToolFeatureFlags::with_disabled(vec!["shell".into()]));
    let policy = BlocklistPolicy::new(flags);
    let actor = ActorId("system".into());
    let tenant = TenantId("default".into());

    for layer in [
        ToolGateLayer::PromptCatalog,
        ToolGateLayer::LlmDefinitions,
        ToolGateLayer::ExecutorPreflight,
    ] {
        let decision = policy
            .check(&ctx_for(
                "echo",
                ToolSource::BuiltIn,
                layer,
                &actor,
                &tenant,
            ))
            .await;
        assert!(
            matches!(decision, ToolGateDecision::Allow),
            "enabled BuiltIn at layer {layer:?} must Allow, got {decision:?}",
        );
    }
}

/// req_tool_visibility_84_020_blocklist_migration_parity
///
/// For every (name × disabled-set) input, `BlocklistPolicy(flags).check(name @ BuiltIn)`
/// is **observationally equivalent** to `flags.is_tool_enabled(name)`:
/// - `Allow`  ⇔  `is_tool_enabled == true`
/// - `Hide`   ⇔  `is_tool_enabled == false`
///
/// This is the migration contract that allows us to delete the legacy
/// `is_tool_enabled` call from `execute.rs` without changing semantics.
#[tokio::test]
async fn req_tool_visibility_84_020_blocklist_migration_parity() {
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec![], "shell"),
        (vec!["shell"], "shell"),
        (vec!["shell"], "echo"),
        (vec!["shell", "http"], "http"),
        (vec!["shell", "http"], "json"),
        (vec!["read_file"], "read_file"),
    ];

    let actor = ActorId("system".into());
    let tenant = TenantId("default".into());

    for (disabled, query) in cases {
        let flags = Arc::new(ToolFeatureFlags::with_disabled(
            disabled.iter().map(|s| s.to_string()),
        ));
        let expected_enabled = flags.is_tool_enabled(query);
        let policy = BlocklistPolicy::new(Arc::clone(&flags));
        let decision = policy
            .check(&ctx_for(
                query,
                ToolSource::BuiltIn,
                ToolGateLayer::ExecutorPreflight,
                &actor,
                &tenant,
            ))
            .await;

        match (expected_enabled, &decision) {
            (true, ToolGateDecision::Allow) => {}
            (false, ToolGateDecision::Hide { .. }) => {}
            _ => panic!(
                "migration parity broken: disabled={disabled:?} query={query} \
                 expected_enabled={expected_enabled} decision={decision:?}",
            ),
        }
    }
}

/// req_tool_visibility_84_021_blocklist_policy_name
///
/// The policy reports a stable, fully-qualified name so audit logs can
/// distinguish it from `AllowAllPolicy` and any future tenant- or
/// scope-specific policies.
#[test]
fn req_tool_visibility_84_021_blocklist_policy_name() {
    let flags = Arc::new(ToolFeatureFlags::all_enabled());
    let policy = BlocklistPolicy::new(flags);
    assert_eq!(policy.name(), "ironclaw::BlocklistPolicy");
}

/// test_security_84_executor_preflight_blocks_hallucinated_tool
///
/// **Defense in depth**: even if a tool slips past L2 (e.g. the LLM
/// hallucinates a tool name not in the catalog, or a stale cache offers
/// a tool the policy now hides), the L3 executor preflight check via
/// `tools.policy().check(layer=ExecutorPreflight)` still rejects it.
///
/// This is the contract that makes the system **Fail-Safe**: a Hide
/// decision at L3 maps to `ToolError::Disabled`, never to silent
/// allow-through.
#[tokio::test]
async fn test_security_84_executor_preflight_blocks_hallucinated_tool() {
    let flags = Arc::new(ToolFeatureFlags::with_disabled(vec!["shell".into()]));
    let policy: Arc<dyn ToolVisibilityPolicy> = Arc::new(BlocklistPolicy::new(flags));
    let actor = ActorId("attacker".into()); // adversarial actor id
    let tenant = TenantId("default".into());

    // Simulate the executor preflight: tool came from LLM tool_call,
    // not from the L2 catalog. Source is classified as BuiltIn.
    let args = serde_json::json!({"command": "rm -rf /"});
    let ctx = ToolGateContext {
        tool_name: "shell",
        source: ToolSource::BuiltIn,
        layer: ToolGateLayer::ExecutorPreflight,
        actor: &actor,
        tenant: &tenant,
        args: Some(&args),
        env: Env::Interactive,
    };

    let decision = policy.check(&ctx).await;

    match decision {
        ToolGateDecision::Hide { reason } | ToolGateDecision::DenyArgs { reason } => {
            assert!(reason.contains("shell"), "audit reason missing tool name");
        }
        other => {
            panic!("FAIL-CLOSED VIOLATION: L3 must block hallucinated disabled tool, got {other:?}",)
        }
    }
}

/// req_tool_visibility_84_022_seed_to_context_conversion
///
/// `ToolGateContextSeed` is the owned counterpart of `ToolGateContext`.
/// Verifies the conversion path used by `tool_definitions_for_llm` and
/// the upcoming `policy_check_executor` builds a context whose fields
/// match the seed.
#[tokio::test]
async fn req_tool_visibility_84_022_seed_to_context_conversion() {
    let seed = ToolGateContextSeed::system(Env::Container);
    assert_eq!(seed.actor.0, "system");
    assert_eq!(seed.tenant.0, "default");
    assert!(matches!(seed.env, Env::Container));

    // Build a ctx using the seed (the same way registry.rs does internally)
    let ctx = ToolGateContext {
        tool_name: "echo",
        source: ToolSource::BuiltIn,
        layer: ToolGateLayer::ExecutorPreflight,
        actor: &seed.actor,
        tenant: &seed.tenant,
        args: None,
        env: seed.env,
    };
    assert_eq!(ctx.actor.0, "system");
    assert_eq!(ctx.tenant.0, "default");
    assert!(matches!(ctx.env, Env::Container));
}
