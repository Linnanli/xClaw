//! In-memory implementation of [`SecretsStore`].
//!
//! Used for tests and as a fallback when no persistent backend is
//! configured. Stored secrets are still encrypted (so `Secret::encrypted_value`
//! round-trips through `SecretsCrypto`) but do not survive a process restart.
//!
//! Ported from `desktop-client/ironclaw/src/secrets/store::in_memory`
//! (F3.2 phase 2 PR 4c, #641) — semantics preserved verbatim so the host
//! shim can re-export this type with zero call-site changes.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use secrecy::ExposeSecret;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::secrets::crypto::SecretsCrypto;
use crate::secrets::store::SecretsStore;
use crate::secrets::types::{CreateSecretParams, DecryptedSecret, Secret, SecretError, SecretRef};

/// In-memory secrets store keyed on `(user_id, name)`.
pub struct InMemorySecretsStore {
    secrets: RwLock<HashMap<(String, String), Secret>>,
    crypto: Arc<SecretsCrypto>,
}

impl InMemorySecretsStore {
    /// Build a new in-memory store backed by the given `SecretsCrypto`.
    pub fn new(crypto: Arc<SecretsCrypto>) -> Self {
        Self {
            secrets: RwLock::new(HashMap::new()),
            crypto,
        }
    }
}

#[async_trait]
impl SecretsStore for InMemorySecretsStore {
    async fn create(
        &self,
        user_id: &str,
        params: CreateSecretParams,
    ) -> Result<Secret, SecretError> {
        let plaintext = params.value.expose_secret().as_bytes();
        let (encrypted_value, key_salt) = self.crypto.encrypt(plaintext)?;

        let now = Utc::now();
        let secret = Secret {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            name: params.name.clone(),
            encrypted_value,
            key_salt,
            provider: params.provider,
            expires_at: params.expires_at,
            last_used_at: None,
            usage_count: 0,
            created_at: now,
            updated_at: now,
        };

        self.secrets
            .write()
            .await
            .insert((user_id.to_string(), params.name), secret.clone());
        Ok(secret)
    }

    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError> {
        let name = name.to_lowercase();
        let secret = self
            .secrets
            .read()
            .await
            .get(&(user_id.to_string(), name.clone()))
            .cloned()
            .ok_or_else(|| SecretError::NotFound(name.clone()))?;

        if let Some(expires_at) = secret.expires_at
            && expires_at < Utc::now()
        {
            return Err(SecretError::Expired);
        }

        Ok(secret)
    }

    async fn get_decrypted(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<DecryptedSecret, SecretError> {
        let secret = self.get(user_id, name).await?;
        self.crypto
            .decrypt(&secret.encrypted_value, &secret.key_salt)
    }

    async fn exists(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        Ok(self
            .secrets
            .read()
            .await
            .contains_key(&(user_id.to_string(), name.to_lowercase())))
    }

    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError> {
        Ok(self
            .secrets
            .read()
            .await
            .iter()
            .filter(|((uid, _), _)| uid == user_id)
            .map(|((_, _), s)| SecretRef {
                name: s.name.clone(),
                provider: s.provider.clone(),
            })
            .collect())
    }

    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        Ok(self
            .secrets
            .write()
            .await
            .remove(&(user_id.to_string(), name.to_lowercase()))
            .is_some())
    }

    async fn record_usage(&self, _secret_id: Uuid) -> Result<(), SecretError> {
        Ok(())
    }

    async fn is_accessible(
        &self,
        user_id: &str,
        secret_name: &str,
        allowed_secrets: &[String],
    ) -> Result<bool, SecretError> {
        let secret_name_lower = secret_name.to_lowercase();
        if !self.exists(user_id, &secret_name_lower).await? {
            return Ok(false);
        }
        for pattern in allowed_secrets {
            let pattern_lower = pattern.to_lowercase();
            if pattern_lower == secret_name_lower {
                return Ok(true);
            }
            if let Some(prefix) = pattern_lower.strip_suffix('*')
                && secret_name_lower.starts_with(prefix)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use secrecy::SecretString;

    use super::*;

    /// 32-byte master key shared by tests. Mirrors `ironclaw::testing::credentials::TEST_CRYPTO_KEY`
    /// so behavior matches existing ironclaw secrets tests.
    const TEST_CRYPTO_KEY: &str = "0123456789abcdef0123456789abcdef";

    fn store() -> InMemorySecretsStore {
        let crypto = Arc::new(SecretsCrypto::new(SecretString::from(TEST_CRYPTO_KEY)).unwrap());
        InMemorySecretsStore::new(crypto)
    }

    fn params(name: &str, value: &str) -> CreateSecretParams {
        CreateSecretParams {
            name: name.to_string(),
            value: SecretString::from(value),
            provider: None,
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_create_get_roundtrip() {
        let s = store();
        s.create("u1", params("api_key", "value-a")).await.unwrap();
        let got = s.get_decrypted("u1", "api_key").await.unwrap();
        assert_eq!(got.expose(), "value-a");
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_per_user_isolation() {
        let s = store();
        s.create("alice", params("token", "alice-val"))
            .await
            .unwrap();
        s.create("bob", params("token", "bob-val")).await.unwrap();
        assert_eq!(
            s.get_decrypted("alice", "token").await.unwrap().expose(),
            "alice-val"
        );
        assert_eq!(
            s.get_decrypted("bob", "token").await.unwrap().expose(),
            "bob-val"
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_security_cross_user_not_found() {
        // Fail-Safe: a user must not be able to read another user's secret,
        // even if they know the name. `get` returns `NotFound`, not the value.
        let s = store();
        s.create("alice", params("k", "secret")).await.unwrap();
        let err = s.get("bob", "k").await.unwrap_err();
        assert!(matches!(err, SecretError::NotFound(_)));
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_expired_rejected() {
        // Fail-Safe: an expired secret must surface `Expired`, not the value.
        let s = store();
        let mut p = params("k", "v");
        p.expires_at = Some(Utc::now() - Duration::seconds(1));
        s.create("u", p).await.unwrap();
        let err = s.get("u", "k").await.unwrap_err();
        assert!(matches!(err, SecretError::Expired));
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_list_scopes_to_user() {
        let s = store();
        s.create("u1", params("a", "1")).await.unwrap();
        s.create("u1", params("b", "2")).await.unwrap();
        s.create("u2", params("c", "3")).await.unwrap();
        let list = s.list("u1").await.unwrap();
        assert_eq!(list.len(), 2);
        let mut names: Vec<_> = list.iter().map(|r| r.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_delete_idempotent() {
        let s = store();
        s.create("u", params("k", "v")).await.unwrap();
        assert!(s.delete("u", "k").await.unwrap());
        assert!(!s.delete("u", "k").await.unwrap());
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_security_is_accessible_exact() {
        let s = store();
        s.create("u", params("github_token", "v")).await.unwrap();
        assert!(
            s.is_accessible("u", "github_token", &["github_token".to_string()])
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_security_is_accessible_glob() {
        let s = store();
        s.create("u", params("github_token", "v")).await.unwrap();
        // Wildcard `github_*` permits `github_token`
        assert!(
            s.is_accessible("u", "github_token", &["github_*".to_string()])
                .await
                .unwrap()
        );
        // But a stricter `slack_*` rule must not match
        assert!(
            !s.is_accessible("u", "github_token", &["slack_*".to_string()])
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_in_memory_security_is_accessible_unknown_secret() {
        // Fail-Safe: even if a permission pattern matches the name, an
        // unknown (uncreated) secret must not be reported as accessible —
        // otherwise a tool could "pre-declare" access and later trigger
        // a write to obtain the secret.
        let s = store();
        assert!(
            !s.is_accessible("u", "ghost", &["*".to_string()])
                .await
                .unwrap()
        );
    }
}
