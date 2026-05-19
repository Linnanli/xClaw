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
//! Concrete `SecretsStore` backends (Postgres / LibSQL / in-memory) and the
//! `SecretsProvider` adapter are intentionally **not** in this crate yet.
//! They will be migrated in follow-up sub-PR 4c of F3.2 phase 2 (#641).

pub mod crypto;
pub mod keychain;
pub mod store;
pub mod types;

pub use crypto::SecretsCrypto;
pub use keychain::KeychainConfig;
pub use store::SecretsStore;
pub use types::{
    CreateSecretParams, CredentialLocation, CredentialMapping, DecryptedSecret, Secret,
    SecretError, SecretRef,
};
