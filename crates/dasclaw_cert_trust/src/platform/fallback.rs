//! Env-var fallback backend.
//!
//! Used on platforms not covered by macOS/Windows/Linux modules. ADR-139
//! §2.C describes the fallback strategy: write `SSL_CERT_FILE` /
//! `REQUESTS_CA_BUNDLE` / `NODE_EXTRA_CA_CERTS` hints to
//! `$CODEX_HOME/proxy/env`. PR1 stubs the operations.

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use super::{TrustStore, not_implemented};
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use crate::Result;
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use crate::status::CertStatus;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
const PLATFORM: &str = "fallback";

/// Env-var fallback backend (no-keychain platforms).
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub struct FallbackTrustStore;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
impl TrustStore for FallbackTrustStore {
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
