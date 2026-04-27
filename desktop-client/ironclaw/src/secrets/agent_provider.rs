//! `x_claw_agent::SecretProvider` adapter for [`SecretsStore`].
//!
//! This adapter lets the agent runtime read user-scoped secrets through
//! ironclaw's encrypted secrets store without depending on ironclaw
//! internals (PostgreSQL pool, crypto keys, user-id plumbing).
//!
//! # Wiring status (Phase 3)
//!
//! - ✅ **Wired for interactive chat** via `Agent::run_agentic_loop`
//!   (uses `hook_bundle_with_safety_and_secrets` with `message.user_id`).
//! - ✅ **Wired for local job worker** via `Worker::execution_loop`
//!   (uses `hook_bundle_with_safety_and_secrets` with `JobContext.user_id`).
//! - ⚠️  **Intentionally NOT wired** for the container worker
//!   (`worker/container.rs`) — it runs inside a Docker container where
//!   secrets are injected at container-provisioning time, not through
//!   the per-call hook bundle.
//!
//! # Scope narrowing (same rationale as Step E)
//!
//! Phase 3 Step F originally called for extracting `src/secrets/` into a
//! separate `crates/ironclaw_secrets/` crate. As with sandbox, the
//! architectural goal (clean `x_claw_agent` boundary) is achieved by
//! the trait seam alone. The store trait (`SecretsStore`) already
//! supports multiple backends (Postgres, LibSql, in-memory), so pulling
//! the crate boundary to a different location adds layout churn without
//! changing dependency direction.
//!
//! # Security posture
//!
//! - Plaintext secrets live briefly in `DecryptedSecret`; this adapter
//!   unwraps them into `SecretString` only inside `get()` and never logs
//!   the value.
//! - `NotFound` is mapped to `Ok(None)` so the agent runtime can treat
//!   "secret not configured" as a first-class non-error.
//! - Every other `SecretError` variant (DecryptionFailed, AccessDenied,
//!   KeychainError, Database, etc.) is surfaced via
//!   `AgentSecretError::Io` so the runtime fails the operation rather
//!   than silently falling through (fail-safe).

use std::sync::Arc;

use async_trait::async_trait;
use x_claw_agent::{SecretError as AgentSecretError, SecretProvider, SecretString};

use crate::secrets::store::SecretsStore;
use crate::secrets::types::SecretError as IronclawSecretError;

/// Adapter exposing [`SecretsStore`] as an `x_claw_agent::SecretProvider`
/// for a single user.
///
/// `user_id` scopes every lookup; the underlying store enforces per-user
/// isolation. The agent runtime is deliberately not exposed to cross-user
/// secret access.
///
/// `S: ?Sized` is required so callers can pass `Arc<dyn SecretsStore + Send + Sync>`
/// (trait-object form used throughout the tool registry).
#[derive(Clone)]
pub struct AgentSecrets<S: SecretsStore + ?Sized + 'static> {
    store: Arc<S>,
    user_id: String,
}

impl<S: SecretsStore + ?Sized + 'static> AgentSecrets<S> {
    pub fn new(store: Arc<S>, user_id: impl Into<String>) -> Self {
        Self {
            store,
            user_id: user_id.into(),
        }
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

#[async_trait]
impl<S: SecretsStore + ?Sized + 'static> SecretProvider for AgentSecrets<S> {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, AgentSecretError> {
        match self.store.get_decrypted(&self.user_id, key).await {
            Ok(decrypted) => Ok(Some(SecretString::new(decrypted.expose().to_string()))),
            Err(IronclawSecretError::NotFound(_)) => Ok(None),
            Err(e) => Err(to_agent_error(e)),
        }
    }

    async fn list_names(&self) -> Result<Vec<String>, AgentSecretError> {
        let refs = self
            .store
            .list(&self.user_id)
            .await
            .map_err(to_agent_error)?;
        Ok(refs.into_iter().map(|r| r.name).collect())
    }
}

fn to_agent_error(err: IronclawSecretError) -> AgentSecretError {
    // `AgentSecretError` only has `NotFound` and `Io`. Everything that
    // isn't NotFound collapses to Io — the runtime cannot meaningfully
    // distinguish "decrypt failed" from "DB unreachable".
    match err {
        IronclawSecretError::NotFound(name) => AgentSecretError::NotFound(name),
        other => AgentSecretError::Io(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::crypto::SecretsCrypto;
    use crate::secrets::store::in_memory::InMemorySecretsStore;
    use crate::secrets::types::CreateSecretParams;
    use secrecy::SecretString as SecrecySecretString;

    const TEST_MASTER_KEY: &str = "0123456789abcdef0123456789abcdef";

    async fn setup() -> AgentSecrets<InMemorySecretsStore> {
        let crypto = Arc::new(
            SecretsCrypto::new(SecrecySecretString::from(TEST_MASTER_KEY.to_string())).unwrap(),
        );
        let store = Arc::new(InMemorySecretsStore::new(crypto));
        store
            .create(
                "user-alice",
                CreateSecretParams::new("openai_key", "sk-test-123"),
            )
            .await
            .unwrap();
        store
            .create(
                "user-alice",
                CreateSecretParams::new("anthropic_key", "sk-ant-xyz"),
            )
            .await
            .unwrap();
        // Another user's secret must be invisible.
        store
            .create(
                "user-bob",
                CreateSecretParams::new("shared_name", "bob-only"),
            )
            .await
            .unwrap();
        AgentSecrets::new(store, "user-alice")
    }

    #[tokio::test]
    async fn get_returns_value_when_secret_exists() {
        let provider = setup().await;
        let got = provider.get("openai_key").await.unwrap();
        let secret = got.expect("secret should exist");
        assert_eq!(secret.expose(), "sk-test-123");
    }

    #[tokio::test]
    async fn get_returns_none_when_secret_missing() {
        let provider = setup().await;
        let got = provider.get("nonexistent_key").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn get_isolates_users() {
        let provider = setup().await;
        // `shared_name` exists for user-bob but not for user-alice.
        let got = provider.get("shared_name").await.unwrap();
        assert!(got.is_none(), "must not leak other user's secret");
    }

    #[tokio::test]
    async fn list_names_returns_only_current_user_secrets() {
        let provider = setup().await;
        let mut names = provider.list_names().await.unwrap();
        names.sort();
        assert_eq!(
            names,
            vec!["anthropic_key".to_string(), "openai_key".to_string()]
        );
    }

    #[tokio::test]
    async fn debug_format_does_not_leak_value() {
        let provider = setup().await;
        let got = provider.get("openai_key").await.unwrap().unwrap();
        let debug = format!("{got:?}");
        assert!(
            !debug.contains("sk-test-123"),
            "Debug impl must not expose secret value; got: {debug}"
        );
    }

    #[test]
    fn not_found_error_roundtrip() {
        let mapped = to_agent_error(IronclawSecretError::NotFound("foo".into()));
        match mapped {
            AgentSecretError::NotFound(name) => assert_eq!(name, "foo"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn non_not_found_errors_collapse_to_io() {
        let mapped = to_agent_error(IronclawSecretError::DecryptionFailed("tamper".into()));
        assert!(
            matches!(mapped, AgentSecretError::Io(_)),
            "non-NotFound errors must fail-safe as Io"
        );
    }
}
