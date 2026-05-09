//! Linux trust-store backend (NSS DB via `certutil` fork-exec).
//!
//! PR1 stub: every method returns [`Error::NotImplemented`]. PR3 (ADR-139
//! §4.5) wires `certutil -d sql:$HOME/.pki/nssdb -A …`.

use super::{TrustStore, not_implemented};
use crate::Result;
use crate::status::CertStatus;

const PLATFORM: &str = "linux";

/// Backend selected on `cfg(target_os = "linux")`.
pub struct LinuxTrustStore;

impl TrustStore for LinuxTrustStore {
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
