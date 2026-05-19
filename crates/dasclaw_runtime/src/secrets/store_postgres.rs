//! PostgreSQL implementation of [`SecretsStore`].
//!
//! Ported verbatim from `desktop-client/ironclaw/src/secrets/store::PostgresSecretsStore`
//! (F3.2 phase 2 PR 4c, #641). The host owns the `Pool` (connection management,
//! TLS configuration, schema migrations); this crate only consumes it.
//!
//! Schema (managed externally, e.g. ironclaw's `migrations/`):
//!
//! ```sql
//! CREATE TABLE secrets (
//!   id              UUID PRIMARY KEY,
//!   user_id         TEXT NOT NULL,
//!   name            TEXT NOT NULL,
//!   encrypted_value BYTEA NOT NULL,
//!   key_salt        BYTEA NOT NULL,
//!   provider        TEXT,
//!   expires_at      TIMESTAMPTZ,
//!   last_used_at    TIMESTAMPTZ,
//!   usage_count     BIGINT NOT NULL DEFAULT 0,
//!   created_at      TIMESTAMPTZ NOT NULL,
//!   updated_at      TIMESTAMPTZ NOT NULL,
//!   UNIQUE(user_id, name)
//! );
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use deadpool_postgres::Pool;
use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::secrets::crypto::SecretsCrypto;
use crate::secrets::store::SecretsStore;
use crate::secrets::types::{CreateSecretParams, DecryptedSecret, Secret, SecretError, SecretRef};

/// PostgreSQL implementation of [`SecretsStore`].
///
/// The pool is owned by the caller — pool sizing, TLS configuration and
/// schema migrations are explicitly out of scope for this crate.
pub struct PostgresSecretsStore {
    pool: Pool,
    crypto: Arc<SecretsCrypto>,
}

impl PostgresSecretsStore {
    /// Build a new store from a pre-configured pool + crypto.
    pub fn new(pool: Pool, crypto: Arc<SecretsCrypto>) -> Self {
        Self { pool, crypto }
    }
}

#[async_trait]
impl SecretsStore for PostgresSecretsStore {
    async fn create(
        &self,
        user_id: &str,
        params: CreateSecretParams,
    ) -> Result<Secret, SecretError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let plaintext = params.value.expose_secret().as_bytes();
        let (encrypted_value, key_salt) = self.crypto.encrypt(plaintext)?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        let row = client
            .query_one(
                r#"
                INSERT INTO secrets (id, user_id, name, encrypted_value, key_salt, provider, expires_at, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                ON CONFLICT (user_id, name) DO UPDATE SET
                    encrypted_value = EXCLUDED.encrypted_value,
                    key_salt = EXCLUDED.key_salt,
                    provider = EXCLUDED.provider,
                    expires_at = EXCLUDED.expires_at,
                    updated_at = NOW()
                RETURNING id, user_id, name, encrypted_value, key_salt, provider, expires_at,
                          last_used_at, usage_count, created_at, updated_at
                "#,
                &[
                    &id,
                    &user_id,
                    &params.name,
                    &encrypted_value,
                    &key_salt,
                    &params.provider,
                    &params.expires_at,
                    &now,
                ],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(row_to_secret(&row))
    }

    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError> {
        let name = name.to_lowercase();
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let row = client
            .query_opt(
                r#"
                SELECT id, user_id, name, encrypted_value, key_salt, provider, expires_at,
                       last_used_at, usage_count, created_at, updated_at
                FROM secrets
                WHERE user_id = $1 AND name = $2
                "#,
                &[&user_id, &name],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        match row {
            Some(r) => {
                let secret = row_to_secret(&r);

                if let Some(expires_at) = secret.expires_at {
                    if expires_at < Utc::now() {
                        return Err(SecretError::Expired);
                    }
                }

                Ok(secret)
            }
            None => Err(SecretError::NotFound(name.to_string())),
        }
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
        let name = name.to_lowercase();
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let row = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM secrets WHERE user_id = $1 AND name = $2)",
                &[&user_id, &name],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(row.get(0))
    }

    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let rows = client
            .query(
                "SELECT name, provider FROM secrets WHERE user_id = $1 ORDER BY name",
                &[&user_id],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| SecretRef {
                name: r.get(0),
                provider: r.get(1),
            })
            .collect())
    }

    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        let name = name.to_lowercase();
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let result = client
            .execute(
                "DELETE FROM secrets WHERE user_id = $1 AND name = $2",
                &[&user_id, &name],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(result > 0)
    }

    async fn record_usage(&self, secret_id: Uuid) -> Result<(), SecretError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        client
            .execute(
                r#"
                UPDATE secrets
                SET last_used_at = NOW(), usage_count = usage_count + 1
                WHERE id = $1
                "#,
                &[&secret_id],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(())
    }

    async fn is_accessible(
        &self,
        user_id: &str,
        secret_name: &str,
        allowed_secrets: &[String],
    ) -> Result<bool, SecretError> {
        let secret_name_lower = secret_name.to_lowercase();
        // Fail-Safe: deny access if the secret doesn't exist, regardless of
        // how permissive the allow-list pattern is. Otherwise a tool could
        // "pre-declare" a wildcard and obtain a future-stored secret.
        if !self.exists(user_id, &secret_name_lower).await? {
            return Ok(false);
        }

        // Glob matching: `openai_*` matches `openai_api_key`.
        for pattern in allowed_secrets {
            let pattern_lower = pattern.to_lowercase();
            if pattern_lower == secret_name_lower {
                return Ok(true);
            }

            if let Some(prefix) = pattern_lower.strip_suffix('*') {
                if secret_name_lower.starts_with(prefix) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

fn row_to_secret(row: &tokio_postgres::Row) -> Secret {
    Secret {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        encrypted_value: row.get("encrypted_value"),
        key_salt: row.get("key_salt"),
        provider: row.get("provider"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
        usage_count: row.get("usage_count"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
