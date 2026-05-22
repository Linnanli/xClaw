//! `SecretsStore` trait — the storage contract any dasclaw host implements.
//!
//! Concrete backends (PostgreSQL, LibSQL, in-memory, future hosts) live in
//! `desktop-client/ironclaw/src/secrets/store.rs` for now; sub-PR 4c of F3.2
//! phase 2 (#641) will migrate them once their cross-crate dependencies
//! (deadpool-postgres / libsql / etc.) are inventoried.
//!
//! Splitting trait from impl now lets:
//! - Any dasclaw host depend on the contract without pulling in DB drivers
//! - Unit tests express expectations against the trait (via in-memory mock)
//! - Future hosts (CLI / headless framework, see ADR-153) implement custom
//!   storage backends without touching ironclaw

use async_trait::async_trait;
use uuid::Uuid;

use super::types::{CreateSecretParams, DecryptedSecret, Secret, SecretError, SecretRef};

/// Trait for secret storage operations.
///
/// Allows for different implementations (PostgreSQL, in-memory for testing).
#[async_trait]
pub trait SecretsStore: Send + Sync {
    /// Store a new secret.
    async fn create(
        &self,
        user_id: &str,
        params: CreateSecretParams,
    ) -> Result<Secret, SecretError>;

    /// Get a secret by name (encrypted form).
    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError>;

    /// Get and decrypt a secret.
    async fn get_decrypted(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<DecryptedSecret, SecretError>;

    /// Check if a secret exists.
    async fn exists(&self, user_id: &str, name: &str) -> Result<bool, SecretError>;

    /// List all secret references for a user (no values).
    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError>;

    /// Delete a secret.
    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError>;

    /// Update secret usage tracking.
    async fn record_usage(&self, secret_id: Uuid) -> Result<(), SecretError>;

    /// Check if a secret is accessible by a tool (based on allowed_secrets).
    async fn is_accessible(
        &self,
        user_id: &str,
        secret_name: &str,
        allowed_secrets: &[String],
    ) -> Result<bool, SecretError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::sync::Mutex;

    /// Minimal in-memory mock that exercises the trait contract.
    ///
    /// This mock is intentionally trivial — it only verifies that the trait
    /// can be implemented and the bound types compose. The real backends
    /// live in ironclaw and have their own behaviour tests.
    struct MockStore {
        secrets: Mutex<Vec<Secret>>,
    }

    #[async_trait]
    impl SecretsStore for MockStore {
        async fn create(
            &self,
            user_id: &str,
            params: CreateSecretParams,
        ) -> Result<Secret, SecretError> {
            let now = Utc::now();
            let s = Secret {
                id: Uuid::new_v4(),
                user_id: user_id.to_string(),
                name: params.name,
                encrypted_value: vec![],
                key_salt: vec![],
                provider: params.provider,
                expires_at: params.expires_at,
                last_used_at: None,
                usage_count: 0,
                created_at: now,
                updated_at: now,
            };
            self.secrets.lock().unwrap().push(s.clone());
            Ok(s)
        }

        async fn get(&self, _user_id: &str, name: &str) -> Result<Secret, SecretError> {
            self.secrets
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.name == name)
                .cloned()
                .ok_or_else(|| SecretError::NotFound(name.to_string()))
        }

        async fn get_decrypted(
            &self,
            _user_id: &str,
            _name: &str,
        ) -> Result<DecryptedSecret, SecretError> {
            Err(SecretError::AccessDenied)
        }

        async fn exists(&self, _user_id: &str, name: &str) -> Result<bool, SecretError> {
            Ok(self.secrets.lock().unwrap().iter().any(|s| s.name == name))
        }

        async fn list(&self, _user_id: &str) -> Result<Vec<SecretRef>, SecretError> {
            Ok(self
                .secrets
                .lock()
                .unwrap()
                .iter()
                .map(|s| SecretRef {
                    name: s.name.clone(),
                    provider: s.provider.clone(),
                })
                .collect())
        }

        async fn delete(&self, _user_id: &str, name: &str) -> Result<bool, SecretError> {
            let mut v = self.secrets.lock().unwrap();
            let len = v.len();
            v.retain(|s| s.name != name);
            Ok(v.len() < len)
        }

        async fn record_usage(&self, _secret_id: Uuid) -> Result<(), SecretError> {
            Ok(())
        }

        async fn is_accessible(
            &self,
            _user_id: &str,
            secret_name: &str,
            allowed_secrets: &[String],
        ) -> Result<bool, SecretError> {
            Ok(allowed_secrets.iter().any(|n| n == secret_name))
        }
    }

    fn store() -> MockStore {
        MockStore {
            secrets: Mutex::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_trait_create_and_exists() {
        let s = store();
        s.create("u1", CreateSecretParams::new("k", "v"))
            .await
            .unwrap();
        assert!(s.exists("u1", "k").await.unwrap());
        assert!(!s.exists("u1", "missing").await.unwrap());
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_trait_list_and_delete() {
        let s = store();
        s.create("u1", CreateSecretParams::new("a", "1"))
            .await
            .unwrap();
        s.create("u1", CreateSecretParams::new("b", "2"))
            .await
            .unwrap();
        assert_eq!(s.list("u1").await.unwrap().len(), 2);
        assert!(s.delete("u1", "a").await.unwrap());
        assert_eq!(s.list("u1").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_trait_is_accessible_default_deny() {
        // Security: empty allowlist denies all access — Fail-Safe, not Fail-Open.
        let s = store();
        s.create("u1", CreateSecretParams::new("k", "v"))
            .await
            .unwrap();
        assert!(!s.is_accessible("u1", "k", &[]).await.unwrap());
        assert!(
            s.is_accessible("u1", "k", &["k".to_string()])
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_secrets_store_trait_get_returns_not_found() {
        let s = store();
        let err = s.get("u1", "missing").await.unwrap_err();
        match err {
            SecretError::NotFound(n) => assert_eq!(n, "missing"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
