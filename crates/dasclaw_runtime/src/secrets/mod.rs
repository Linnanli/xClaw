//! Secret-management vocabulary for any dasclaw host.
//!
//! This module hosts the **portable surface** of the secrets subsystem:
//! - [`types`]: pure data types (`Secret`, `SecretRef`, `DecryptedSecret`,
//!   `SecretError`, `CreateSecretParams`, `CredentialLocation`,
//!   `CredentialMapping`).
//! - [`store`]: the `SecretsStore` trait — the abstract storage contract
//!   that any backend (PostgreSQL, LibSQL, in-memory, future hosts) must
//!   implement.
//! - [`crypto`]: AES-256-GCM + HKDF-SHA256 encryption/decryption
//!   ([`SecretsCrypto`]). Pure crypto layer, no OS/brand coupling.
//! - [`keychain`]: OS keychain integration ([`KeychainConfig`]). Hosts
//!   declare their own `const KeychainConfig` to namespace storage —
//!   the framework crate itself contains no host-brand literals.
//!
//! Concrete storage backends (in-memory, PostgreSQL, libSQL) and the
//! [`dasclaw_core::SecretProvider`] adapter live in [`store_in_memory`],
//! [`store_postgres`] (feature `postgres`), [`store_libsql`] (feature
//! `libsql`), and [`agent_provider`]. Migrated in F3.2 phase 2 PR 4c (#641).

pub mod agent_provider;
pub mod crypto;
pub mod keychain;
pub mod store;
pub mod store_in_memory;
pub mod types;

#[cfg(feature = "postgres")]
pub mod store_postgres;

#[cfg(feature = "libsql")]
pub mod store_libsql;

pub use agent_provider::AgentSecrets;
pub use crypto::SecretsCrypto;
pub use keychain::KeychainConfig;
pub use store::SecretsStore;
pub use store_in_memory::InMemorySecretsStore;
pub use types::{
    CreateSecretParams, CredentialLocation, CredentialMapping, DecryptedSecret, Secret,
    SecretError, SecretRef,
};

#[cfg(feature = "postgres")]
pub use store_postgres::PostgresSecretsStore;

#[cfg(feature = "libsql")]
pub use store_libsql::LibSqlSecretsStore;
