//! Ironclaw-specific keychain namespace shim.
//!
//! The portable [`KeychainConfig`] type and all platform back-ends live in
//! [`dasclaw_runtime::secrets::keychain`]. This file just pins the host brand
//! (`"ironclaw"`) and re-exports the four async operations as zero-arg free
//! functions, so the 18 existing call sites in this crate stay verbatim.
//!
//! See ADR-152 phase 2 PR 4b' (#641) for the rationale: the framework crate
//! must not contain host-brand literals — every host declares its own
//! [`KeychainConfig`] const.

use dasclaw_runtime::secrets::keychain::KeychainConfig;
use dasclaw_runtime::secrets::types::SecretError;

/// The ironclaw host's keychain namespace.
///
/// Service name `"ironclaw"` + account `"master_key"` — identical to the
/// pre-migration constants, so existing OS keychain entries continue to
/// resolve without any user-visible change.
pub const IRONCLAW_KEYCHAIN: KeychainConfig = KeychainConfig::new("ironclaw", "master_key");

// Re-export the brand-free helpers; they need no namespace.
pub use dasclaw_runtime::secrets::keychain::{generate_master_key, generate_master_key_hex};

/// Store the master key in the OS keychain under the ironclaw namespace.
pub async fn store_master_key(key: &[u8]) -> Result<(), SecretError> {
    IRONCLAW_KEYCHAIN.store_master_key(key).await
}

/// Retrieve the master key from the OS keychain under the ironclaw namespace.
pub async fn get_master_key() -> Result<Vec<u8>, SecretError> {
    IRONCLAW_KEYCHAIN.get_master_key().await
}

/// Delete the master key from the OS keychain under the ironclaw namespace.
pub async fn delete_master_key() -> Result<(), SecretError> {
    IRONCLAW_KEYCHAIN.delete_master_key().await
}

/// Check whether the master key exists in the OS keychain under the ironclaw
/// namespace.
pub async fn has_master_key() -> bool {
    IRONCLAW_KEYCHAIN.has_master_key().await
}
