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
//!
//! OS-keychain integration and concrete storage backends are intentionally
//! **not** in this crate (yet). They live in
//! `desktop-client/ironclaw/src/secrets/` and will be migrated in follow-up
//! sub-PRs 4b' (keychain — needs `service_name` parameterization to avoid
//! leaking the host brand) and 4c (Postgres / LibSQL / InMemory impls) of
//! F3.2 phase 2 (#641).

pub mod crypto;
pub mod store;
pub mod types;

pub use crypto::SecretsCrypto;
pub use store::SecretsStore;
pub use types::{
    CreateSecretParams, CredentialLocation, CredentialMapping, DecryptedSecret, Secret,
    SecretError, SecretRef,
};
