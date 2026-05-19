//! Secret types — re-export shim.
//!
//! As of F3.2 phase 2 PR 4a (#641) the canonical types live in
//! [`dasclaw_runtime::secrets::types`] so any dasclaw host shares the same
//! vocabulary. This module is preserved as a thin re-export to avoid
//! churning the dozens of `crate::secrets::types::*` import paths that
//! exist inside ironclaw; new code should prefer the upstream path.

pub use dasclaw_runtime::secrets::types::{
    CreateSecretParams, CredentialLocation, CredentialMapping, DecryptedSecret, Secret,
    SecretError, SecretRef,
};
