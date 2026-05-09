//! MITM CA trust-chain manager — ADR-139 implementation skeleton.
//!
//! This crate centralizes per-user CA install / uninstall / rotation logic
//! used by `dasclaw_net_proxy`'s MITM TLS audit path. It is **dasclaw
//! original code** (not a verbatim port from codex upstream — codex does
//! not provide an equivalent).
//!
//! # Status (PR1 / ADR-139 §4.5)
//!
//! All platform-specific entry points currently return
//! [`Error::NotImplemented`]. PR2 (macOS + Windows keychain) and PR3
//! (Linux NSS DB) will replace those stubs with real implementations.
//!
//! # API
//!
//! The five public functions correspond 1:1 with the `dasclaw cert`
//! CLI subcommands documented in ADR-139 §4.3:
//!
//! - [`install_ca`] — write the CA into the per-user trust store
//! - [`uninstall_ca`] — remove the CA and clear `$CODEX_HOME/proxy/`
//! - [`rotate_ca`] — regenerate the CA and re-inject
//! - [`status`] — report CA fingerprint / expiry / store state
//! - [`export_ca_pem`] — return the raw PEM bytes for manual import

#![deny(missing_docs)]

pub mod error;
pub mod platform;
pub mod status;

pub use error::Error;
pub use status::CertStatus;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Install the dasclaw MITM CA into the current user's OS trust store.
///
/// `ca_pem` is the PEM-encoded CA certificate produced by `dasclaw_net_proxy`.
/// Returns `Ok(())` once the CA is registered. On platforms that lack a
/// supported trust store, falls back to writing env-var hints to
/// `$CODEX_HOME/proxy/env`.
pub fn install_ca(ca_pem: &[u8]) -> Result<()> {
    platform::current().install(ca_pem)
}

/// Remove the dasclaw MITM CA from the current user's trust store.
///
/// Idempotent: returns `Ok(())` even if no CA is currently installed.
pub fn uninstall_ca() -> Result<()> {
    platform::current().uninstall()
}

/// Rotate the CA: uninstall the previous certificate and install `new_ca_pem`.
///
/// Equivalent to `uninstall_ca` followed by `install_ca`, but implemented as
/// a single call so PR2/PR3 can perform it atomically per-platform.
pub fn rotate_ca(new_ca_pem: &[u8]) -> Result<()> {
    platform::current().rotate(new_ca_pem)
}

/// Report the current CA install state.
pub fn status() -> Result<CertStatus> {
    platform::current().status()
}

/// Return the PEM bytes of the currently-installed CA, if any.
///
/// Useful for `dasclaw cert export` (manual Firefox / NSS import).
pub fn export_ca_pem() -> Result<Vec<u8>> {
    platform::current().export()
}
