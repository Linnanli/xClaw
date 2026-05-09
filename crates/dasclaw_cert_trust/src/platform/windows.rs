//! Windows trust-store backend (`CurrentUser\Root` via `windows-rs`).
//!
//! PR1 stub: every method returns [`Error::NotImplemented`]. PR2 wires
//! `windows::Win32::Security::Cryptography` calls.

use super::{TrustStore, not_implemented};
use crate::Result;
use crate::status::CertStatus;

const PLATFORM: &str = "windows";

/// Backend selected on `cfg(target_os = "windows")`.
pub struct WindowsTrustStore;

impl TrustStore for WindowsTrustStore {
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
