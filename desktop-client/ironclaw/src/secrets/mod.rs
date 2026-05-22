//! Ironclaw host-specific secrets glue.
//!
//! All cryptographic primitives, storage backends, and the
//! `SecretsStore`/`SecretsCrypto`/`AgentSecrets` types live in
//! [`dasclaw_runtime::secrets`]. This module only retains the
//! ironclaw-specific bits that the framework crate must not contain:
//!
//! - [`keychain`] — the `IRONCLAW_KEYCHAIN` brand constant and zero-arg
//!   wrappers over [`dasclaw_runtime::secrets::keychain`].
//! - [`create_secrets_store`] — wires `DatabaseHandles` (an ironclaw-only
//!   type) into the runtime stores.
//! - [`resolve_master_key`] — reads the env var or queries the ironclaw
//!   keychain namespace.
//! - [`crypto_from_hex`] — convenience constructor used by the desktop
//!   bootstrap path.
//!
//! See ADR-152 §3 (F4.3) for the rationale: the framework crate is host
//! neutral, so anything that mentions ironclaw branding or desktop-only
//! types stays in this module.
//!
//! # Master Key Storage
//!
//! The master key for encrypting secrets can come from:
//! - **OS Keychain** (recommended for local installs): auto-generated
//!   and stored under the `"ironclaw"` namespace.
//! - **Environment variable** (for CI/Docker): set `SECRETS_MASTER_KEY`.

pub mod keychain;

use dasclaw_runtime::secrets::{SecretError, SecretsCrypto, SecretsStore};

/// Create a secrets store from a master key and database handles.
///
/// Returns `None` if no matching backend handle is available (e.g. when
/// running without a database). This is a normal condition in no-db mode,
/// not an error — callers should treat `None` as "secrets unavailable".
pub fn create_secrets_store(
    crypto: std::sync::Arc<SecretsCrypto>,
    handles: &crate::db::DatabaseHandles,
) -> Option<std::sync::Arc<dyn SecretsStore + Send + Sync>> {
    let store: Option<std::sync::Arc<dyn SecretsStore + Send + Sync>> = None;

    #[cfg(feature = "libsql")]
    let store = store.or_else(|| {
        handles.libsql_db.as_ref().map(|db| {
            std::sync::Arc::new(dasclaw_runtime::secrets::LibSqlSecretsStore::new(
                std::sync::Arc::clone(db),
                std::sync::Arc::clone(&crypto),
            )) as std::sync::Arc<dyn SecretsStore + Send + Sync>
        })
    });

    #[cfg(feature = "postgres")]
    let store = store.or_else(|| {
        handles.pg_pool.as_ref().map(|pool| {
            std::sync::Arc::new(dasclaw_runtime::secrets::PostgresSecretsStore::new(
                pool.clone(),
                std::sync::Arc::clone(&crypto),
            )) as std::sync::Arc<dyn SecretsStore + Send + Sync>
        })
    });

    store
}

/// Try to resolve an existing master key from env var or OS keychain.
///
/// Resolution order:
/// 1. `SECRETS_MASTER_KEY` environment variable (hex-encoded)
/// 2. OS keychain (macOS Keychain / Linux secret-service)
///
/// Returns `None` if no key is available (caller should generate one).
pub async fn resolve_master_key() -> Option<String> {
    // 1. Check env var
    if let Ok(env_key) = std::env::var("SECRETS_MASTER_KEY")
        && !env_key.is_empty()
    {
        return Some(env_key);
    }

    // 2. Try OS keychain
    if let Ok(keychain_key_bytes) = keychain::get_master_key().await {
        let key_hex: String = keychain_key_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        return Some(key_hex);
    }

    None
}

/// Create a `SecretsCrypto` from a master key string.
///
/// The key is typically hex-encoded (from `generate_master_key_hex` or
/// the `SECRETS_MASTER_KEY` env var), but `SecretsCrypto::new` validates
/// only key length, not encoding. Any sufficiently long string works.
pub fn crypto_from_hex(hex: &str) -> Result<std::sync::Arc<SecretsCrypto>, SecretError> {
    let crypto = SecretsCrypto::new(secrecy::SecretString::from(hex.to_string()))?;
    Ok(std::sync::Arc::new(crypto))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_from_hex_valid() {
        // 32 bytes = 64 hex chars
        let hex = "0123456789abcdef".repeat(4); // 64 hex chars
        let result = crypto_from_hex(&hex);
        assert!(result.is_ok()); // safety: test assertion
    }

    #[test]
    fn test_crypto_from_hex_invalid() {
        let result = crypto_from_hex("too_short");
        assert!(result.is_err()); // safety: test assertion
    }
}
