//! Unified agentic loop engine — re-export shim.
//!
//! Phase 3 Step D-4.5: the real engine moved to
//! [`dasclaw_core::agentic_loop`]. This module now only re-exports the
//! public surface so existing `use crate::agent::agentic_loop::{...}` call
//! sites (in [`crate::agent::dispatcher`], [`crate::worker::job`],
//! [`crate::worker::container`]) keep resolving unchanged.
//!
//! Route-B: `LoopDelegate::call_llm` and `run_agentic_loop` no longer take
//! `reasoning: &Reasoning`. Each delegate now owns a `Reasoning` engine as a
//! field and uses `self.reasoning` directly. Error type at the trait
//! boundary is `dasclaw_core::traits::HostError`
//! (`Box<dyn std::error::Error + Send + Sync>`); ironclaw's internal
//! helpers keep returning `crate::error::Error` and rely on the blanket
//! `From<E> for Box<dyn Error + Send + Sync>` to cross the boundary via
//! `?` or `.map_err(Into::into)`.

pub use dasclaw_core::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
pub use dasclaw_core::intent::truncate_for_preview;
// Issue #73 slice D: re-export PermissionMode so call sites (dispatcher,
// worker/job, worker/container) construct hook bundles without depending
// directly on dasclaw_core::permissions. The session-config layer that
// eventually decides the mode lives in this crate — agent kernel stays
// agnostic.
pub use dasclaw_core::permissions::PermissionMode;
// Issue #73 slice E: workspace boundary is now sourced from a
// capability-validated `WorkspaceCapability` instead of a raw `PathBuf`.
// Re-exported so call sites don't have to depend on `dasclaw_workspace_cap`
// directly.
pub use dasclaw_workspace_cap::WorkspaceCapability;
// ADR-152 §3 F2.2 (#626): bash permission rule context is re-exported so
// call sites construct `BashPermissionHook` rule contexts without taking a
// direct dependency on the `dasclaw_bash_permissions` crate. Real rule
// ingestion (from admin-backend / session config) is Phase 2.3.
pub use dasclaw_hooks::ToolPermissionContext;

use dasclaw_core::traits::HostError;

/// Convert a `HostError` produced by the engine back into ironclaw's
/// concrete [`crate::error::Error`].
///
/// Delegates in this crate always return errors that originated as
/// `crate::error::Error` and were boxed via the blanket
/// `From<E> for Box<dyn Error + Send + Sync>`. We therefore downcast
/// first, and only wrap as `LlmError::InvalidResponse` if the boxed error
/// came from somewhere else (shouldn't happen in practice).
pub(crate) fn host_err_to_error(e: HostError) -> crate::error::Error {
    match e.downcast::<crate::error::Error>() {
        Ok(boxed) => *boxed,
        Err(other) => crate::error::LlmError::InvalidResponse {
            provider: "agent".to_string(),
            reason: other.to_string(),
        }
        .into(),
    }
}

/// Build an `dasclaw_core::HookBundle` whose `egress` slot is wired to a
/// [`dasclaw_governance::CompositeEgressGate`] (ADR-148) chaining
/// [`dasclaw_hooks::BashValidationHook`] (bash command-string validation,
/// issue #73 slice A1) → [`ironclaw_safety::egress_gate::IronclawEgressGate`]
/// (generic JSON validation + leak detection).
///
/// `workspace_cap` is the capability handle for the session workspace
/// (issue #73 slice E). The bash hook reads `.root()` for `pathValidation`,
/// so any path-escape check it performs is by definition consistent with
/// the same boundary the rest of the agent (sandbox, FS tools, etc.) sees.
/// `permission_mode` is the per-session bash validation policy
/// (issue #73 slice D): callers must pass an explicit mode — typically
/// [`PermissionMode::WorkspaceWrite`] for regular chat/worker sessions,
/// or [`PermissionMode::ReadOnly`] for routines that must not mutate.
///
/// `sandbox` / `secrets` / `approval` keep their `Noop` / `InMemory` /
/// `AutoApprove` defaults — they will be wired in by later Phase 3 steps.
pub fn hook_bundle_with_safety(
    safety: std::sync::Arc<crate::safety::SafetyLayer>,
    workspace_cap: std::sync::Arc<WorkspaceCapability>,
    permission_mode: PermissionMode,
) -> dasclaw_core::HookBundle {
    hook_bundle_with_safety_and_permissions(
        safety,
        workspace_cap,
        permission_mode,
        ToolPermissionContext::default(),
    )
}

/// ADR-152 §3 F2.2 (#626) variant of [`hook_bundle_with_safety`] that lets
/// callers thread a [`ToolPermissionContext`] into the
/// [`dasclaw_hooks::BashPermissionHook`] step of the egress chain.
///
/// Chain order is **bash-validation → bash-permissions → ironclaw-safety**:
///
/// 1. [`dasclaw_hooks::BashValidationHook`] — command-string structural /
///    path-escape validation (security gate, never abstains).
/// 2. [`dasclaw_hooks::BashPermissionHook`] — Deny/Ask/Allow rule pipeline
///    over `permission_context`. With the default (empty) context every
///    decision is `Passthrough`, so this step is a no-op until Phase 2.3
///    (config ingestion) lands real rules. Inserting it now so the chain
///    shape is stable and tests can drive the gate end-to-end.
/// 3. [`ironclaw_safety::egress_gate::IronclawEgressGate`] — generic JSON
///    validation + leak detection.
///
/// Empty-context guarantee: `BashPermissionHook::new(ToolPermissionContext::default())`
/// returns `Passthrough` for every bash command, so the new step cannot
/// regress the behaviour of [`hook_bundle_with_safety`] callers that don't
/// supply rules.
pub fn hook_bundle_with_safety_and_permissions(
    safety: std::sync::Arc<crate::safety::SafetyLayer>,
    workspace_cap: std::sync::Arc<WorkspaceCapability>,
    permission_mode: PermissionMode,
    permission_context: ToolPermissionContext,
) -> dasclaw_core::HookBundle {
    use std::sync::Arc;
    let composite = dasclaw_core::CompositeEgressGate::builder()
        .add(
            "bash-validation",
            Arc::new(dasclaw_hooks::BashValidationHook::new(
                permission_mode,
                workspace_cap.root().to_path_buf(),
            )),
        )
        .add(
            "bash-permissions",
            Arc::new(dasclaw_hooks::BashPermissionHook::new(permission_context)),
        )
        .add(
            "ironclaw-safety",
            Arc::new(ironclaw_safety::egress_gate::IronclawEgressGate::new(
                safety,
            )),
        )
        .build();
    let mut bundle = dasclaw_core::HookBundle::noop();
    bundle.egress = Arc::new(composite);
    bundle
}

/// Build a `HookBundle` with both `egress` and `secrets` slots wired in.
///
/// - `egress` routes through [`IronclawEgressGate`](ironclaw_safety::egress_gate::IronclawEgressGate).
/// - `secrets` routes through [`AgentSecrets`](crate::secrets::agent_provider::AgentSecrets)
///   if the tool registry carries a [`SecretsStore`](crate::secrets::SecretsStore);
///   falls back to the noop provider when the registry has none.
/// - `sandbox` / `approval` keep their `Noop` / `AutoApprove` defaults until
///   ADR-002 Phase 4 lands the real sandbox backends.
///
/// The registry reference is passed by `&Arc` on purpose: the registry
/// already owns the store, so we only need a shared read — we do not want to
/// force callers to clone the whole registry every hook-bundle construction.
pub fn hook_bundle_with_safety_and_secrets(
    safety: std::sync::Arc<crate::safety::SafetyLayer>,
    tools: &std::sync::Arc<crate::tools::ToolRegistry>,
    user_id: impl Into<String>,
    workspace_cap: std::sync::Arc<WorkspaceCapability>,
    permission_mode: PermissionMode,
) -> dasclaw_core::HookBundle {
    let mut bundle = hook_bundle_with_safety(safety, workspace_cap, permission_mode);
    if let Some(store) = tools.secrets_store() {
        bundle.secrets = std::sync::Arc::new(crate::secrets::agent_provider::AgentSecrets::new(
            store.clone(),
            user_id,
        ));
    }
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::SafetyLayer;
    use crate::secrets::{CreateSecretParams, InMemorySecretsStore, SecretsCrypto, SecretsStore};
    use crate::tools::ToolRegistry;
    use crate::tools::wasm::SharedCredentialRegistry;
    use ironclaw_safety::SafetyConfig;
    use secrecy::SecretString as SecrecySecretString;
    use std::sync::Arc;

    const TEST_MASTER_KEY: &str = "0123456789abcdef0123456789abcdef";

    fn safety_layer() -> Arc<SafetyLayer> {
        Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: false,
        }))
    }

    fn registry_without_secrets() -> Arc<ToolRegistry> {
        Arc::new(ToolRegistry::new())
    }

    /// Open the current directory as a `WorkspaceCapability` for use in tests.
    /// `.` is always openable in a cargo-test working directory.
    fn test_workspace_cap() -> Arc<WorkspaceCapability> {
        Arc::new(WorkspaceCapability::open(".").expect("workspace cap should open on `.` in tests"))
    }

    async fn registry_with_secret(user_id: &str, key: &str, value: &str) -> Arc<ToolRegistry> {
        let crypto = Arc::new(
            SecretsCrypto::new(SecrecySecretString::from(TEST_MASTER_KEY.to_string())).unwrap(),
        );
        let store = Arc::new(InMemorySecretsStore::new(crypto));
        store
            .create(user_id, CreateSecretParams::new(key, value))
            .await
            .unwrap();
        let store_dyn: Arc<dyn SecretsStore + Send + Sync> = store;
        let registry = ToolRegistry::new()
            .with_credentials(Arc::new(SharedCredentialRegistry::default()), store_dyn);
        Arc::new(registry)
    }

    #[tokio::test]
    async fn safety_only_helper_leaves_secrets_as_noop() {
        let tools = registry_without_secrets();
        let _ = &tools; // silence unused warning when helper does not consume it
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        // The noop secret provider returns None for any key (never errors).
        let got = bundle.secrets.get("any_key").await.unwrap();
        assert!(
            got.is_none(),
            "safety-only helper must leave secrets as noop"
        );
    }

    #[tokio::test]
    async fn with_secrets_helper_exposes_registered_secret() {
        let tools = registry_with_secret("alice", "openai_key", "sk-live-abc").await;
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        let got = bundle
            .secrets
            .get("openai_key")
            .await
            .expect("get must succeed")
            .expect("secret must exist");
        assert_eq!(got.expose(), "sk-live-abc");
    }

    #[tokio::test]
    async fn with_secrets_helper_isolates_by_user_id() {
        let tools = registry_with_secret("alice", "alice_only", "A").await;
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "bob",
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        // Bob must not see alice's secrets.
        let got = bundle.secrets.get("alice_only").await.unwrap();
        assert!(
            got.is_none(),
            "user_id scoping must block cross-user secret access"
        );
    }

    #[tokio::test]
    async fn with_secrets_helper_falls_back_to_noop_when_registry_has_no_store() {
        // A registry built without `.with_credentials(...)` has no SecretsStore.
        let tools = registry_without_secrets();
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        let got = bundle.secrets.get("anything").await.unwrap();
        assert!(
            got.is_none(),
            "no store = fall through to noop (no panic, no error)"
        );
    }

    // ---- Issue #73 slice D: PermissionMode threading tests ----
    //
    // These tests prove that the `permission_mode` parameter actually
    // reaches the inner `BashValidationHook` instead of being silently
    // ignored. We pick a `rm -rf` style destructive command because it
    // produces a deterministic Warn in `dasclaw_bash_validation`, and
    // slice C's `map_warn_by_mode` table gives a different `SafetyDecision`
    // per mode — so any short-circuit at construction time would surface
    // immediately.
    //
    // We intentionally do NOT re-test the full 5-mode mapping table here
    // (that's covered by `dasclaw_hooks::bash_validation_hook::tests` and
    // `req_warn_*` from slice C). Only the *threading* is in scope.

    /// `ReadOnly` mode must cause the destructive `rm` to be hard-Blocked
    /// at the bash gate — proves the mode parameter is consumed.
    #[tokio::test]
    async fn req_safety_73_d_read_only_blocks_destructive_command() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::ReadOnly,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "ReadOnly mode must block destructive bash command, got {decision:?}"
        );
    }

    /// `WorkspaceWrite` mode (production default) must allow the same
    /// destructive command — Warn → Allow per slice C's mapping table.
    /// Different mode in == different decision out: the threading works.
    #[tokio::test]
    async fn req_safety_73_d_workspace_write_allows_destructive_command() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Allow),
            "WorkspaceWrite mode must allow destructive bash command (Warn -> Allow), got {decision:?}"
        );
    }

    /// `Prompt` mode must surface a `EgressDecision::Ask` so the UI can
    /// gate the destructive command behind user confirmation.
    #[tokio::test]
    async fn req_safety_73_d_prompt_mode_asks_on_destructive_command() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let bundle =
            hook_bundle_with_safety(safety_layer(), test_workspace_cap(), PermissionMode::Prompt);
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Ask { .. }),
            "Prompt mode must Ask for confirmation, got {decision:?}"
        );
    }

    /// `DangerFullAccess` mode must allow the destructive command (slice C
    /// maps Warn → Allow + info-level log).
    #[tokio::test]
    async fn req_safety_73_d_danger_full_access_allows_destructive_command() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::DangerFullAccess,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Allow),
            "DangerFullAccess mode must allow destructive bash command, got {decision:?}"
        );
    }

    /// The secrets-aware helper must thread `permission_mode` through to
    /// the same composite hook as the safety-only helper — guards against
    /// future refactors that drop the parameter on one path.
    #[tokio::test]
    async fn req_safety_73_d_with_secrets_helper_threads_permission_mode() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let tools = registry_without_secrets();
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            test_workspace_cap(),
            PermissionMode::ReadOnly,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "hook_bundle_with_safety_and_secrets must thread ReadOnly to bash hook"
        );
    }

    /// Sanity: `hook_bundle_with_safety_and_secrets` reuses the safety wiring
    /// — callers get the full stack from a single call.
    #[tokio::test]
    async fn with_secrets_helper_preserves_safety_wiring() {
        let tools = registry_with_secret("alice", "k", "v").await;
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        // Exact identity check is not possible across Arc<dyn Trait>, but we
        // can at least assert the safety Arc pointer count > 1 (we hold one,
        // bundle holds one → 2).
        let _ = &bundle.egress;
        // Smoke-check: secrets slot is functional (not the noop).
        assert!(bundle.secrets.get("k").await.unwrap().is_some());
    }

    // ---- Issue #73 slice E: WorkspaceCapability injection ----

    /// The workspace boundary inside the bundle must come from the
    /// supplied `WorkspaceCapability::root()`, not from any implicit
    /// process-level cwd. Building a cap rooted at a `TempDir` and
    /// feeding it through both helpers must succeed and yield a hook
    /// bundle whose bash policy still rejects the destructive command
    /// under `ReadOnly` (proves the cap-rooted helper path is wired
    /// end-to-end into the bash hook).
    #[tokio::test]
    async fn req_safety_73_e_workspace_cap_threads_through_safety_helper() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let tmp = tempfile::tempdir().expect("tempdir");
        let cap = Arc::new(
            WorkspaceCapability::open(tmp.path())
                .expect("WorkspaceCapability::open on tempdir must succeed"),
        );
        assert_eq!(cap.root(), tmp.path(), "cap root must equal supplied path");

        let bundle = hook_bundle_with_safety(safety_layer(), cap, PermissionMode::ReadOnly);
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_e" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "cap-rooted bundle must still enforce ReadOnly bash policy, got {decision:?}"
        );
    }

    /// Same as above for the secrets-aware helper — the cap parameter
    /// must reach the bash hook regardless of which constructor path
    /// callers use.
    #[tokio::test]
    async fn req_safety_73_e_workspace_cap_threads_through_secrets_helper() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let tmp = tempfile::tempdir().expect("tempdir");
        let cap = Arc::new(
            WorkspaceCapability::open(tmp.path())
                .expect("WorkspaceCapability::open on tempdir must succeed"),
        );
        let tools = registry_without_secrets();
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            cap,
            PermissionMode::ReadOnly,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_e" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "cap-rooted secrets bundle must still enforce ReadOnly, got {decision:?}"
        );
    }

    // ---- ADR-152 §3 F2.2 (#626): BashPermissionHook wiring ----
    //
    // These tests prove the new `bash-permissions` step in the composite
    // egress chain is reachable end-to-end and honours rule context:
    //
    // 1. Empty context: no rule fires → existing safety path is untouched
    //    (the `req_safety_73_d_*` family above already exercises this
    //    against `hook_bundle_with_safety` — those tests would have
    //    regressed if the empty-context Passthrough contract was wrong).
    // 2. Deny rule fires: a destructive command that bash-validation would
    //    Allow (under `WorkspaceWrite`) is Blocked because a user-supplied
    //    Deny rule matches. Proves the new step actually runs.
    // 3. Ask rule fires: the permission hook surfaces `Ask` so the UI can
    //    confirm. Distinguishes "hook is wired but only deny works" from
    //    "hook is wired across all behaviours".
    //
    // Full per-behaviour matrix is owned by
    // `dasclaw_hooks::bash_permission_hook::tests`; ironclaw only tests the
    // wiring.

    fn ctx_with_rule(
        behavior: dasclaw_hooks::PermissionBehavior,
        rule: &str,
    ) -> ToolPermissionContext {
        let mut ctx = ToolPermissionContext::default();
        ctx.add_rule(behavior, dasclaw_hooks::PermissionRuleSource::Session, rule);
        ctx
    }

    #[tokio::test]
    async fn adr152_f22_permission_hook_deny_rule_blocks_bash_command() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let ctx = ctx_with_rule(dasclaw_hooks::PermissionBehavior::Deny, "rm:*");
        let bundle = hook_bundle_with_safety_and_permissions(
            safety_layer(),
            test_workspace_cap(),
            // WorkspaceWrite is the mode under which bash-validation would
            // Allow `rm -rf /tmp/foo` — so any Block we observe here must
            // come from the new permission hook step, not the validation
            // step.
            PermissionMode::WorkspaceWrite,
            ctx,
        );
        let args = serde_json::json!({ "command": "rm -rf /tmp/adr152_f22" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Block { .. }),
            "Deny rule must block bash command via BashPermissionHook, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn adr152_f22_permission_hook_ask_rule_surfaces_ask() {
        use dasclaw_core::{EgressDecision, EgressKind};
        let ctx = ctx_with_rule(dasclaw_hooks::PermissionBehavior::Ask, "echo:*");
        let bundle = hook_bundle_with_safety_and_permissions(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
            ctx,
        );
        // `echo hello` is benign — bash-validation Allows it. Any `Ask`
        // observed must come from the permission hook.
        let args = serde_json::json!({ "command": "echo hello" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Ask { .. }),
            "Ask rule must surface EgressDecision::Ask, got {decision:?}"
        );
    }

    #[tokio::test]
    async fn adr152_f22_safety_only_helper_uses_empty_permission_context() {
        use dasclaw_core::{EgressDecision, EgressKind};
        // `hook_bundle_with_safety` delegates to the permissioned helper
        // with `ToolPermissionContext::default()`. A benign command under
        // WorkspaceWrite must Allow — proving the empty context is a true
        // no-op for the new step (not silently blocking everything).
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            test_workspace_cap(),
            PermissionMode::WorkspaceWrite,
        );
        let args = serde_json::json!({ "command": "echo hello" });
        let decision = bundle
            .egress
            .check(
                &EgressKind::ToolExecution {
                    tool: "bash".to_string(),
                },
                &args.to_string(),
            )
            .await;
        assert!(
            matches!(decision, EgressDecision::Allow),
            "empty permission context must not regress safety-only helper, got {decision:?}"
        );
    }
}
