//! Re-export shim: `SecretsCrypto` lives in [`dasclaw_runtime::secrets::crypto`].
//!
//! Migrated to the shared framework crate in F3.2 phase 2 PR 4b (#641).
//! This file is intentionally a one-line re-export so existing callers keep
//! working (`crate::secrets::crypto::SecretsCrypto` and the re-export
//! `crate::secrets::SecretsCrypto`).

pub use dasclaw_runtime::secrets::crypto::SecretsCrypto;
