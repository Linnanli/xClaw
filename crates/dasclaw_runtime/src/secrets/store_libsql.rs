//! libSQL/Turso implementation of [`SecretsStore`].
//!
//! Ported verbatim from `desktop-client/ironclaw/src/secrets/store::LibSqlSecretsStore`
//! (F3.2 phase 2 PR 4c, #641). The host owns the `Arc<libsql::Database>`
//! handle (replication, remote sync, TLS); we create a fresh connection per
//! operation, matching ironclaw's `LibSqlBackend` connection-per-request
//! pattern and avoiding shared-mutable-state hazards inside the store.
//!
//! Storage encodings (mirroring ironclaw's schema):
//! - timestamps: RFC3339 with millisecond precision, stored as `TEXT`
//! - `encrypted_value`, `key_salt`: stored as `BLOB`
//! - `provider`, `expires_at`, `last_used_at`: `TEXT NULL`
//! - `usage_count`: `INTEGER NOT NULL DEFAULT 0`

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::secrets::crypto::SecretsCrypto;
use crate::secrets::store::SecretsStore;
use crate::secrets::types::{CreateSecretParams, DecryptedSecret, Secret, SecretError, SecretRef};

/// libSQL/Turso implementation of [`SecretsStore`].
pub struct LibSqlSecretsStore {
    db: Arc<libsql::Database>,
    crypto: Arc<SecretsCrypto>,
}

impl LibSqlSecretsStore {
    /// Build a new store from a shared libsql database handle + crypto.
    pub fn new(db: Arc<libsql::Database>, crypto: Arc<SecretsCrypto>) -> Self {
        Self { db, crypto }
    }

    async fn connect(&self) -> Result<libsql::Connection, SecretError> {
        let conn = self
            .db
            .connect()
            .map_err(|e| SecretError::Database(format!("Connection failed: {e}")))?;
        conn.query("PRAGMA busy_timeout = 5000", ())
            .await
            .map_err(|e| SecretError::Database(format!("Failed to set busy_timeout: {e}")))?;
        Ok(conn)
    }
}

#[async_trait]
impl SecretsStore for LibSqlSecretsStore {
    async fn create(
        &self,
        user_id: &str,
        params: CreateSecretParams,
    ) -> Result<Secret, SecretError> {
        let plaintext = params.value.expose_secret().as_bytes();
        let (encrypted_value, key_salt) = self.crypto.encrypt(plaintext)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let expires_at_str = params
            .expires_at
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));

        // Atomic upsert + read-back inside a transaction.
        let conn = self.connect().await?;
        let tx = conn
            .transaction()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        tx.execute(
                r#"
                INSERT INTO secrets (id, user_id, name, encrypted_value, key_salt, provider, expires_at, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                ON CONFLICT (user_id, name) DO UPDATE SET
                    encrypted_value = excluded.encrypted_value,
                    key_salt = excluded.key_salt,
                    provider = excluded.provider,
                    expires_at = excluded.expires_at,
                    updated_at = ?8
                "#,
                libsql::params![
                    id.to_string(),
                    user_id,
                    params.name.as_str(),
                    libsql::Value::Blob(encrypted_value.clone()),
                    libsql::Value::Blob(key_salt.clone()),
                    libsql_opt_text(params.provider.as_deref()),
                    libsql_opt_text(expires_at_str.as_deref()),
                    now_str.as_str(),
                ],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let mut rows = tx
            .query(
                r#"
                SELECT id, user_id, name, encrypted_value, key_salt, provider, expires_at,
                       last_used_at, usage_count, created_at, updated_at
                FROM secrets
                WHERE user_id = ?1 AND name = ?2
                "#,
                libsql::params![user_id, params.name.as_str()],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?
            .ok_or_else(|| SecretError::Database("Insert succeeded but row not found".into()))?;

        let secret = libsql_row_to_secret(&row)?;

        tx.commit()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(secret)
    }

    async fn get(&self, user_id: &str, name: &str) -> Result<Secret, SecretError> {
        let name = name.to_lowercase();
        let conn = self.connect().await?;
        let mut rows = conn
            .query(
                r#"
                SELECT id, user_id, name, encrypted_value, key_salt, provider, expires_at,
                       last_used_at, usage_count, created_at, updated_at
                FROM secrets
                WHERE user_id = ?1 AND name = ?2
                "#,
                libsql::params![user_id, name.as_str()],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        match rows
            .next()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?
        {
            Some(row) => {
                let secret = libsql_row_to_secret(&row)?;

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
        let conn = self.connect().await?;
        let mut rows = conn
            .query(
                "SELECT 1 FROM secrets WHERE user_id = ?1 AND name = ?2",
                libsql::params![user_id, name.as_str()],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(rows
            .next()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?
            .is_some())
    }

    async fn list(&self, user_id: &str) -> Result<Vec<SecretRef>, SecretError> {
        let conn = self.connect().await?;
        let mut rows = conn
            .query(
                "SELECT name, provider FROM secrets WHERE user_id = ?1 ORDER BY name",
                libsql::params![user_id],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        let mut refs = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?
        {
            refs.push(SecretRef {
                name: row.get::<String>(0).unwrap_or_default(),
                provider: row.get::<String>(1).ok(),
            });
        }
        Ok(refs)
    }

    async fn delete(&self, user_id: &str, name: &str) -> Result<bool, SecretError> {
        let name = name.to_lowercase();
        let conn = self.connect().await?;
        let affected = conn
            .execute(
                "DELETE FROM secrets WHERE user_id = ?1 AND name = ?2",
                libsql::params![user_id, name.as_str()],
            )
            .await
            .map_err(|e| SecretError::Database(e.to_string()))?;

        Ok(affected > 0)
    }

    async fn record_usage(&self, secret_id: Uuid) -> Result<(), SecretError> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let conn = self.connect().await?;

        conn.execute(
            r#"
                UPDATE secrets
                SET last_used_at = ?1, usage_count = usage_count + 1
                WHERE id = ?2
                "#,
            libsql::params![now.as_str(), secret_id.to_string()],
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
        // how permissive the allow-list pattern is.
        if !self.exists(user_id, &secret_name_lower).await? {
            return Ok(false);
        }

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

fn libsql_opt_text(s: Option<&str>) -> libsql::Value {
    match s {
        Some(s) => libsql::Value::Text(s.to_string()),
        None => libsql::Value::Null,
    }
}

fn libsql_parse_timestamp(s: &str) -> Result<chrono::DateTime<Utc>, SecretError> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Ok(ndt.and_utc());
    }
    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(ndt.and_utc());
    }
    Err(SecretError::Database(format!(
        "unparseable timestamp: {s:?}"
    )))
}

fn libsql_row_to_secret(row: &libsql::Row) -> Result<Secret, SecretError> {
    let id_str: String = row
        .get(0)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let user_id: String = row
        .get(1)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let name: String = row
        .get(2)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let encrypted_value: Vec<u8> = row
        .get(3)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let key_salt: Vec<u8> = row
        .get(4)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let provider: Option<String> = row.get::<String>(5).ok().filter(|s| !s.is_empty());
    let expires_at = row
        .get::<String>(6)
        .ok()
        .filter(|s| !s.is_empty())
        .and_then(|s| libsql_parse_timestamp(&s).ok());
    let last_used_at = row
        .get::<String>(7)
        .ok()
        .filter(|s| !s.is_empty())
        .and_then(|s| libsql_parse_timestamp(&s).ok());
    let usage_count: i64 = row.get::<i64>(8).unwrap_or(0);
    let created_at_str: String = row
        .get(9)
        .map_err(|e| SecretError::Database(e.to_string()))?;
    let updated_at_str: String = row
        .get(10)
        .map_err(|e| SecretError::Database(e.to_string()))?;

    Ok(Secret {
        id: id_str
            .parse()
            .map_err(|e: uuid::Error| SecretError::Database(e.to_string()))?,
        user_id,
        name,
        encrypted_value,
        key_salt,
        provider,
        expires_at,
        last_used_at,
        usage_count,
        created_at: libsql_parse_timestamp(&created_at_str)?,
        updated_at: libsql_parse_timestamp(&updated_at_str)?,
    })
}
