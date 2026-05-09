//! macOS trust-store backend (per-user keychain via `security-framework`).
//!
//! PR1 stub: every method returns [`Error::NotImplemented`]. PR2 (ADR-139
//! §4.5) wires `SecKeychain*` calls.

use super::{TrustStore, not_implemented};
use crate::Result;
use crate::status::CertStatus;

const PLATFORM: &str = "macos";

/// Backend selected on `cfg(target_os = "macos")`.
pub struct MacOsTrustStore;

impl TrustStore for MacOsTrustStore {
    fn platform(&self) -> &'static str {
        PLATFORM
    }

    fn install(&self, _ca_pem: &[u8]) -> Result<()> {
        not_implemented("install", PLATFORM)
    }

    fn uninstall(&self) -> Result<()> {
        not_implemented("uninstall", PLATFORM)
    }

    fn status(&self) -> Result<CertStatus> {
        Ok(CertStatus::not_installed(PLATFORM))
    }

    fn export(&self) -> Result<Vec<u8>> {
        not_implemented("export", PLATFORM)
    }
}
