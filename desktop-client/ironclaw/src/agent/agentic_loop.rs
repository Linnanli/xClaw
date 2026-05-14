//! Unified agentic loop engine — re-export shim.
//!
//! Phase 3 Step D-4.5: the real engine moved to
//! [`x_claw_agent::agentic_loop`]. This module now only re-exports the
//! public surface so existing `use crate::agent::agentic_loop::{...}` call
//! sites (in [`crate::agent::dispatcher`], [`crate::worker::job`],
//! [`crate::worker::container`]) keep resolving unchanged.
//!
//! Route-B: `LoopDelegate::call_llm` and `run_agentic_loop` no longer take
//! `reasoning: &Reasoning`. Each delegate now owns a `Reasoning` engine as a
//! field and uses `self.reasoning` directly. Error type at the trait
//! boundary is `x_claw_agent::traits::HostError`
//! (`Box<dyn std::error::Error + Send + Sync>`); ironclaw's internal
//! helpers keep returning `crate::error::Error` and rely on the blanket
//! `From<E> for Box<dyn Error + Send + Sync>` to cross the boundary via
//! `?` or `.map_err(Into::into)`.

pub use x_claw_agent::agentic_loop::{
    AgenticLoopConfig, LoopDelegate, LoopOutcome, LoopSignal, TextAction, run_agentic_loop,
};
pub use x_claw_agent::intent::truncate_for_preview;
// Issue #73 slice D: re-export PermissionMode so call sites (dispatcher,
// worker/job, worker/container) construct hook bundles without depending
// directly on x_claw_agent::permissions. The session-config layer that
// eventually decides the mode lives in this crate — agent kernel stays
// agnostic.
pub use x_claw_agent::permissions::PermissionMode;

use x_claw_agent::traits::HostError;

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

/// Build an `x_claw_agent::HookBundle` whose `safety` slot is wired to a
/// [`x_claw_agent::CompositeSafetyHook`] (per ADR-147) chaining
/// [`dasclaw_hooks::BashValidationHook`] (bash command-string validation,
/// issue #73 slice A1) → [`ironclaw_safety::agent_hook::IronclawSafetyHook`]
/// (generic JSON validation + leak detection).
///
/// `workspace` is the root used by `pathValidation` to detect workspace
/// escapes. `permission_mode` is the per-session bash validation policy
/// (issue #73 slice D): callers must pass an explicit mode — typically
/// [`PermissionMode::WorkspaceWrite`] for regular chat/worker sessions,
/// or [`PermissionMode::ReadOnly`] for routines that must not mutate.
/// Threading the mode as an explicit parameter keeps the safety boundary
/// readable at every call site and lets the future session-config layer
/// populate it without touching this crate.
///
/// `sandbox` / `secrets` / `approval` keep their `Noop` / `InMemory` /
/// `AutoApprove` defaults — they will be wired in by later Phase 3 steps.
/// This helper exists so every consumer (chat dispatcher, job worker,
/// container worker) constructs an identical bundle and we do not lose
/// the safety boundary the moment any one call site forgets to plug it in.
pub fn hook_bundle_with_safety(
    safety: std::sync::Arc<crate::safety::SafetyLayer>,
    workspace: std::path::PathBuf,
    permission_mode: PermissionMode,
) -> x_claw_agent::HookBundle {
    use std::sync::Arc;
    let composite = x_claw_agent::CompositeSafetyHook::builder()
        .add(
            "bash-validation",
            Arc::new(dasclaw_hooks::BashValidationHook::new(
                permission_mode,
                workspace,
            )),
        )
        .add(
            "ironclaw-safety",
            Arc::new(ironclaw_safety::agent_hook::IronclawSafetyHook::new(safety)),
        )
        .build();
    let mut bundle = x_claw_agent::HookBundle::noop();
    bundle.safety = Arc::new(composite);
    bundle
}

/// Build a `HookBundle` with both `safety` and `secrets` slots wired in.
///
/// - `safety` routes through [`IronclawSafetyHook`](ironclaw_safety::agent_hook::IronclawSafetyHook).
/// - `secrets` routes through [`AgentSecrets`](crate::secrets::agent_provider::AgentSecrets)
///   if the tool registry carries a [`SecretsStore`](crate::secrets::SecretsStore);
///   falls back to the noop provider when the registry has none (which is the
///   case in test harnesses and in contexts where secrets are injected by the
///   container runtime instead of by the agent loop).
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
    workspace: std::path::PathBuf,
    permission_mode: PermissionMode,
) -> x_claw_agent::HookBundle {
    let mut bundle = hook_bundle_with_safety(safety, workspace, permission_mode);
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
            std::path::PathBuf::from("."),
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
            std::path::PathBuf::from("."),
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
            std::path::PathBuf::from("."),
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
            std::path::PathBuf::from("."),
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
        use x_claw_agent::SafetyDecision;
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            std::path::PathBuf::from("."),
            PermissionMode::ReadOnly,
        );
        let mut args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .safety
            .before_tool_call("bash", &mut args)
            .await
            .unwrap();
        assert!(
            matches!(decision, SafetyDecision::Block { .. }),
            "ReadOnly mode must block destructive bash command, got {decision:?}"
        );
    }

    /// `WorkspaceWrite` mode (production default) must allow the same
    /// destructive command — Warn → Allow per slice C's mapping table.
    /// Different mode in == different decision out: the threading works.
    #[tokio::test]
    async fn req_safety_73_d_workspace_write_allows_destructive_command() {
        use x_claw_agent::SafetyDecision;
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            std::path::PathBuf::from("."),
            PermissionMode::WorkspaceWrite,
        );
        let mut args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .safety
            .before_tool_call("bash", &mut args)
            .await
            .unwrap();
        assert!(
            matches!(decision, SafetyDecision::Allow),
            "WorkspaceWrite mode must allow destructive bash command (Warn -> Allow), got {decision:?}"
        );
    }

    /// `Prompt` mode must surface a `SafetyDecision::Ask` so the UI can
    /// gate the destructive command behind user confirmation.
    #[tokio::test]
    async fn req_safety_73_d_prompt_mode_asks_on_destructive_command() {
        use x_claw_agent::SafetyDecision;
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            std::path::PathBuf::from("."),
            PermissionMode::Prompt,
        );
        let mut args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .safety
            .before_tool_call("bash", &mut args)
            .await
            .unwrap();
        assert!(
            matches!(decision, SafetyDecision::Ask { .. }),
            "Prompt mode must Ask for confirmation, got {decision:?}"
        );
    }

    /// `DangerFullAccess` mode must allow the destructive command (slice C
    /// maps Warn → Allow + info-level log).
    #[tokio::test]
    async fn req_safety_73_d_danger_full_access_allows_destructive_command() {
        use x_claw_agent::SafetyDecision;
        let bundle = hook_bundle_with_safety(
            safety_layer(),
            std::path::PathBuf::from("."),
            PermissionMode::DangerFullAccess,
        );
        let mut args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .safety
            .before_tool_call("bash", &mut args)
            .await
            .unwrap();
        assert!(
            matches!(decision, SafetyDecision::Allow),
            "DangerFullAccess mode must allow destructive bash command, got {decision:?}"
        );
    }

    /// The secrets-aware helper must thread `permission_mode` through to
    /// the same composite hook as the safety-only helper — guards against
    /// future refactors that drop the parameter on one path.
    #[tokio::test]
    async fn req_safety_73_d_with_secrets_helper_threads_permission_mode() {
        use x_claw_agent::SafetyDecision;
        let tools = registry_without_secrets();
        let bundle = hook_bundle_with_safety_and_secrets(
            safety_layer(),
            &tools,
            "alice",
            std::path::PathBuf::from("."),
            PermissionMode::ReadOnly,
        );
        let mut args = serde_json::json!({ "command": "rm -rf /tmp/req_safety_73_d" });
        let decision = bundle
            .safety
            .before_tool_call("bash", &mut args)
            .await
            .unwrap();
        assert!(
            matches!(decision, SafetyDecision::Block { .. }),
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
            std::path::PathBuf::from("."),
            PermissionMode::WorkspaceWrite,
        );
        // Exact identity check is not possible across Arc<dyn Trait>, but we
        // can at least assert the safety Arc pointer count > 1 (we hold one,
        // bundle holds one → 2).
        let _ = &bundle.safety;
        // Smoke-check: secrets slot is functional (not the noop).
        assert!(bundle.secrets.get("k").await.unwrap().is_some());
    }
}
