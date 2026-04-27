//! Integration contract test: the three `x_claw_agent` Hook trait adapters
//! provided by ironclaw compose correctly.
//!
//! This test is the **contract proof** for Phase 3 Steps G/E/F:
//!
//! - `IronclawSafetyHook`     (ironclaw_safety::agent_hook)   — impl SafetyHook
//! - `SandboxAgentExecutor`   (ironclaw::sandbox::agent_executor) — impl SandboxExecutor
//! - `AgentSecrets<S>`        (ironclaw::secrets::agent_provider) — impl SecretProvider
//!
//! Goals:
//!
//! 1. Prove the three adapters can be instantiated and passed through
//!    `Arc<dyn Trait>` — i.e. the object-safe trait seam actually works.
//! 2. Prove end-to-end behaviour through the trait (not through the
//!    concrete ironclaw types) for the happy paths the agent runtime will
//!    exercise once D-5 is revisited.
//! 3. Lock in fail-safe behaviour (secret leaks blocked, cross-user
//!    isolation, workspace escape rejected) at the trait boundary.
//!
//! Scope:
//! - Pure in-memory fixtures. No Docker, no database. The sandbox
//!   adapter is exercised via its file-IO paths (which the Phase 3 Step
//!   E adapter implements natively, without Docker). `run_bash` is
//!   intentionally NOT invoked here — that path requires Docker and
//!   lives in existing E2E tests.
//!
//! If any of these assertions drift, the Phase 3 architectural boundary
//! has silently regressed and must be investigated.

use std::path::PathBuf;
use std::sync::Arc;

use ironclaw::sandbox::agent_executor::SandboxAgentExecutor;
use ironclaw::sandbox::config::SandboxConfig;
use ironclaw::sandbox::manager::SandboxManager;
use ironclaw::secrets::agent_provider::AgentSecrets;
use ironclaw::secrets::{CreateSecretParams, InMemorySecretsStore, SecretsCrypto, SecretsStore};
use ironclaw_safety::agent_hook::IronclawSafetyHook;
use ironclaw_safety::{SafetyConfig, SafetyLayer};
use secrecy::SecretString as SecrecySecretString;
use serde_json::json;
use x_claw_agent::{
    ApprovalGate, AutoApproveGate, SafetyDecision, SafetyHook, SandboxExecutor, SecretProvider,
};

const TEST_MASTER_KEY: &str = "0123456789abcdef0123456789abcdef";

/// Fixture bundle mirroring what a future `agent_app.rs` would assemble
/// when Phase 3 D-5 is revisited.
struct HookBundle {
    safety: Arc<dyn SafetyHook>,
    sandbox: Arc<dyn SandboxExecutor>,
    secrets: Arc<dyn SecretProvider>,
    approval: Arc<dyn ApprovalGate>,
    workspace: tempfile::TempDir,
}

async fn build_bundle() -> HookBundle {
    // SafetyHook: wrap ironclaw's SafetyLayer.
    let safety_layer = Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 4_096,
        injection_check_enabled: true,
    }));
    let safety: Arc<dyn SafetyHook> = Arc::new(IronclawSafetyHook::new(safety_layer));

    // SandboxExecutor: wrap a SandboxManager bound to a tempdir workspace.
    // The manager is not initialize()d — run_bash would fail, but the
    // read_file/write_file paths do not need it.
    let workspace = tempfile::tempdir().expect("create temp workspace");
    let manager = Arc::new(SandboxManager::new(SandboxConfig::default()));
    let sandbox: Arc<dyn SandboxExecutor> = Arc::new(SandboxAgentExecutor::new(
        manager,
        workspace.path().to_path_buf(),
    ));

    // SecretProvider: in-memory store + a test user with two secrets.
    let crypto = Arc::new(
        SecretsCrypto::new(SecrecySecretString::from(TEST_MASTER_KEY.to_string()))
            .expect("crypto init"),
    );
    let store = Arc::new(InMemorySecretsStore::new(crypto));
    store
        .create(
            "user-alice",
            CreateSecretParams::new("openai_key", "sk-alice-1"),
        )
        .await
        .unwrap();
    store
        .create(
            "user-bob",
            CreateSecretParams::new("openai_key", "sk-bob-1"),
        )
        .await
        .unwrap();
    let secrets: Arc<dyn SecretProvider> = Arc::new(AgentSecrets::new(store, "user-alice"));

    // ApprovalGate: x_claw_agent's built-in AutoApprove is good enough
    // here — we just need to prove the wiring compiles.
    let approval: Arc<dyn ApprovalGate> = Arc::new(AutoApproveGate);

    HookBundle {
        safety,
        sandbox,
        secrets,
        approval,
        workspace,
    }
}

// ---------------------------------------------------------------------------
// Contract: SafetyHook
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contract_safety_hook_allows_clean_prompt() {
    let b = build_bundle().await;
    let mut prompt = "list files in cwd".to_string();
    let decision = b.safety.before_prompt(&mut prompt).await.unwrap();
    assert_eq!(decision, SafetyDecision::Allow);
    assert_eq!(prompt, "list files in cwd");
}

#[tokio::test]
async fn contract_safety_hook_blocks_prompt_with_leaked_credential() {
    let b = build_bundle().await;
    let mut prompt = format!("here is my key: sk-{}", "A".repeat(48));
    let decision = b.safety.before_prompt(&mut prompt).await.unwrap();
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "prompt containing a leaked secret must be blocked, got {decision:?}"
    );
}

#[tokio::test]
async fn contract_safety_hook_redacts_secret_in_completion() {
    let b = build_bundle().await;
    let original = format!("token is sk-{}", "A".repeat(48));
    let mut completion = original.clone();
    b.safety.after_completion(&mut completion).await.unwrap();
    assert_ne!(
        completion, original,
        "completion containing a secret must be modified"
    );
}

#[tokio::test]
async fn contract_safety_hook_sanitizes_tool_output_truncation() {
    // Build a fresh bundle with a tight output limit so truncation fires.
    let safety_layer = Arc::new(SafetyLayer::new(&SafetyConfig {
        max_output_length: 32,
        injection_check_enabled: false,
    }));
    let hook: Arc<dyn SafetyHook> = Arc::new(IronclawSafetyHook::new(safety_layer));

    let mut output = "a".repeat(500);
    hook.after_tool_output("bash", &mut output).await.unwrap();
    assert!(
        output.contains("truncated"),
        "oversize tool output must be truncated with a notice; got len={}",
        output.len()
    );
}

// ---------------------------------------------------------------------------
// Contract: SandboxExecutor
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contract_sandbox_write_read_roundtrip_inside_workspace() {
    let b = build_bundle().await;
    let rel = PathBuf::from("sub/hello.txt");
    b.sandbox.write_file(&rel, b"hi").await.unwrap();
    let got = b.sandbox.read_file(&rel).await.unwrap();
    assert_eq!(got, b"hi");
}

#[tokio::test]
async fn contract_sandbox_rejects_write_outside_workspace() {
    let b = build_bundle().await;
    let err = b
        .sandbox
        .write_file(&PathBuf::from("/tmp/escape.txt"), b"x")
        .await
        .unwrap_err();
    assert!(
        matches!(err, x_claw_agent::SandboxError::PolicyViolation(_)),
        "writes outside workspace root must return PolicyViolation, got {err:?}"
    );
}

#[tokio::test]
async fn contract_sandbox_rejects_parent_escape() {
    let b = build_bundle().await;
    let err = b
        .sandbox
        .read_file(&PathBuf::from("../../../etc/passwd"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, x_claw_agent::SandboxError::PolicyViolation(_)),
        "parent-dir escape must return PolicyViolation, got {err:?}"
    );
    // And the target workspace must still be intact after the rejected call.
    assert!(b.workspace.path().exists());
}

// ---------------------------------------------------------------------------
// Contract: SecretProvider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contract_secrets_returns_value_for_configured_key() {
    let b = build_bundle().await;
    let got = b.secrets.get("openai_key").await.unwrap();
    let secret = got.expect("openai_key must be configured for user-alice");
    assert_eq!(secret.expose(), "sk-alice-1");
}

#[tokio::test]
async fn contract_secrets_returns_none_for_missing_key() {
    let b = build_bundle().await;
    let got = b.secrets.get("not_configured").await.unwrap();
    assert!(
        got.is_none(),
        "missing secret must map to Ok(None), not Err"
    );
}

#[tokio::test]
async fn contract_secrets_never_leaks_across_users() {
    // Build a second bundle scoped to user-bob through a FRESH store, but
    // seed both users so we know user-alice's provider cannot see bob.
    let crypto = Arc::new(
        SecretsCrypto::new(SecrecySecretString::from(TEST_MASTER_KEY.to_string())).unwrap(),
    );
    let store = Arc::new(InMemorySecretsStore::new(crypto));
    store
        .create(
            "user-alice",
            CreateSecretParams::new("team_token", "alice-owned"),
        )
        .await
        .unwrap();
    store
        .create(
            "user-bob",
            CreateSecretParams::new("team_token", "bob-owned"),
        )
        .await
        .unwrap();

    let alice: Arc<dyn SecretProvider> = Arc::new(AgentSecrets::new(store.clone(), "user-alice"));
    let bob: Arc<dyn SecretProvider> = Arc::new(AgentSecrets::new(store, "user-bob"));

    let a = alice.get("team_token").await.unwrap().unwrap();
    let b = bob.get("team_token").await.unwrap().unwrap();
    assert_eq!(a.expose(), "alice-owned");
    assert_eq!(b.expose(), "bob-owned");
    assert_ne!(a.expose(), b.expose());
}

#[tokio::test]
async fn contract_secrets_list_names_is_user_scoped() {
    let b = build_bundle().await;
    let names = b.secrets.list_names().await.unwrap();
    assert_eq!(names, vec!["openai_key".to_string()]);
}

// ---------------------------------------------------------------------------
// Contract: ApprovalGate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contract_approval_gate_is_object_safe_and_callable() {
    let b = build_bundle().await;
    // We only prove the trait object is usable — behavior of AutoApprove
    // is not ironclaw-specific.
    let req = x_claw_agent::ApprovalRequest {
        tool: "bash".to_string(),
        args: json!({"cmd": "ls"}),
        description: "smoke".to_string(),
        allow_always: false,
    };
    let outcome = b.approval.request(req).await.unwrap();
    assert_eq!(
        outcome,
        x_claw_agent::ApprovalOutcome::Approved,
        "AutoApproveGate must approve",
    );
}

// ---------------------------------------------------------------------------
// Composition: all four adapters coexist in one `Arc<dyn>` bundle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn contract_all_four_hooks_compose_and_are_send_sync() {
    // Move the bundle across task boundaries to prove Arc<dyn Trait> stays
    // Send + Sync, which is the key invariant for the future agent loop.
    let b = build_bundle().await;
    let handle = tokio::spawn(async move {
        let mut prompt = "hi".to_string();
        let d = b.safety.before_prompt(&mut prompt).await.unwrap();
        assert_eq!(d, SafetyDecision::Allow);
        let names = b.secrets.list_names().await.unwrap();
        assert_eq!(names.len(), 1);
        b.sandbox
            .write_file(&PathBuf::from("touch.txt"), b"ok")
            .await
            .unwrap();
    });
    handle.await.expect("spawned task completed");
}
