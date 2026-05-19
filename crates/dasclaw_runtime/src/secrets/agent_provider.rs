//! [`dasclaw_core::SecretProvider`] adapter for [`SecretsStore`].
//!
//! Ported from `desktop-client/ironclaw/src/secrets/agent_provider.rs`
//! (F3.2 phase 2 PR 4c, #641). The adapter is pure glue — no host-specific
//! knowledge — so any dasclaw host that uses both `dasclaw_core` (for the
//! agent runtime / hooks trait) and `dasclaw_runtime::secrets::SecretsStore`
//! (for encrypted multi-user storage) can wire them together without
//! reimplementing the same ~100 lines.
//!
//! # Security posture
//!
//! - Plaintext secrets only live briefly inside `DecryptedSecret`; this
//!   adapter unwraps them into `SecretString` only inside [`get`] and never
//!   logs the value.
//! - `NotFound` is mapped to `Ok(None)` so the agent runtime can treat
//!   "secret not configured" as a first-class non-error.
//! - Every other [`SecretError`] variant (`DecryptionFailed`, `AccessDenied`,
//!   `KeychainError`, `Database`, …) is surfaced via
//!   [`dasclaw_core::SecretError::Io`] so the runtime fails the operation
//!   rather than silently falling through (Fail-Safe over Fail-Open).
//!
//! [`get`]: AgentSecrets::get

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::{SecretError as AgentSecretError, SecretProvider, SecretString};

use crate::secrets::store::SecretsStore;
use crate::secrets::types::SecretError;

/// Adapter exposing [`SecretsStore`] as a [`dasclaw_core::SecretProvider`]
/// scoped to a single user.
///
/// `user_id` scopes every lookup; the underlying store enforces per-user
/// isolation. The agent runtime is deliberately not exposed to cross-user
/// secret access.
///
/// `S: ?Sized` is required so callers can pass
/// `Arc<dyn SecretsStore + Send + Sync>` (the trait-object form used
/// throughout the host's tool registry).
#[derive(Clone)]
pub struct AgentSecrets<S: SecretsStore + ?Sized + 'static> {
    store: Arc<S>,
    user_id: String,
}

impl<S: SecretsStore + ?Sized + 'static> AgentSecrets<S> {
    /// Build a new adapter scoped to `user_id`.
    pub fn new(store: Arc<S>, user_id: impl Into<String>) -> Self {
        Self {
            store,
            user_id: user_id.into(),
        }
    }

    /// The user this adapter is scoped to.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

#[async_trait]
impl<S: SecretsStore + ?Sized + 'static> SecretProvider for AgentSecrets<S> {
    async fn get(&self, key: &str) -> Result<Option<SecretString>, AgentSecretError> {
        match self.store.get_decrypted(&self.user_id, key).await {
            Ok(decrypted) => Ok(Some(SecretString::new(decrypted.expose().to_string()))),
            Err(SecretError::NotFound(_)) => Ok(None),
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

fn to_agent_error(err: SecretError) -> AgentSecretError {
    // `AgentSecretError` only has `NotFound` and `Io`. Everything that
    // isn't `NotFound` collapses to `Io` — the runtime cannot meaningfully
    // distinguish "decrypt failed" from "DB unreachable", so a single
    // failure variant keeps the surface minimal and the behavior Fail-Safe.
    match err {
        SecretError::NotFound(name) => AgentSecretError::NotFound(name),
        other => AgentSecretError::Io(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString as SecrecySecretString;

    use super::*;
    use crate::secrets::crypto::SecretsCrypto;
    use crate::secrets::store_in_memory::InMemorySecretsStore;
    use crate::secrets::types::CreateSecretParams;

    /// 32-byte test master key. Same value as ironclaw's `TEST_CRYPTO_KEY`
    /// so cross-checking against the host shim's tests is straightforward.
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
    async fn req_dasclaw_runtime_secrets_agent_provider_get_returns_value() {
        let provider = setup().await;
        let got = provider.get("openai_key").await.unwrap();
        let secret = got.expect("secret should exist");
        assert_eq!(secret.expose(), "sk-test-123");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_agent_provider_get_missing_returns_none() {
        let provider = setup().await;
        let got = provider.get("nonexistent_key").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_agent_provider_security_isolates_users() {
        // Cross-user isolation: even if user-bob owns `shared_name`,
        // user-alice's provider must report `None`, not the value.
        let provider = setup().await;
        let got = provider.get("shared_name").await.unwrap();
        assert!(got.is_none(), "must not leak other user's secret");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_agent_provider_list_names_scopes_to_user() {
        let provider = setup().await;
        let mut names = provider.list_names().await.unwrap();
        names.sort();
        assert_eq!(
            names,
            vec!["anthropic_key".to_string(), "openai_key".to_string()]
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_agent_provider_security_debug_redacts_value() {
        // Defense-in-depth: even if a caller logs the returned SecretString,
        // its Debug impl must not include the plaintext.
        let provider = setup().await;
        let got = provider.get("openai_key").await.unwrap().unwrap();
        let debug = format!("{got:?}");
        assert!(
            !debug.contains("sk-test-123"),
            "Debug impl must not expose secret value; got: {debug}"
        );
    }

    #[test]
    fn req_dasclaw_runtime_secrets_agent_provider_not_found_passthrough() {
        let mapped = to_agent_error(SecretError::NotFound("foo".into()));
        match mapped {
            AgentSecretError::NotFound(name) => assert_eq!(name, "foo"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_runtime_secrets_agent_provider_security_non_not_found_fails_safe() {
        // Fail-Safe: any error other than NotFound (decrypt failed, DB
        // unreachable, access denied) MUST surface as Io and abort the
        // runtime's operation — never silently produce `Ok(None)`, which
        // would let a tool proceed without its required credentials.
        let mapped = to_agent_error(SecretError::DecryptionFailed("tamper".into()));
        assert!(
            matches!(mapped, AgentSecretError::Io(_)),
            "non-NotFound errors must fail-safe as Io"
        );
    }
}
