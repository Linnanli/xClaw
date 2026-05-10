//! Per-platform trust-store backends.
//!
//! PR1 ships only the dispatch trait and stub implementations; PR2 fills in
//! macOS / Windows keychain code and PR3 adds Linux NSS DB.

use crate::Result;
#[cfg(any(
    target_os = "linux",
    not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
))]
use crate::error::Error;
use crate::status::CertStatus;

mod fallback;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// Trust-store backend interface.
///
/// Each platform module implements [`TrustStore`] for a unit struct and the
/// active platform is selected by [`current`].
pub trait TrustStore {
    /// Platform name reported by `status()` and `Error::NotImplemented`.
    fn platform(&self) -> &'static str;

    /// Install a CA into the trust store.
    fn install(&self, ca_pem: &[u8]) -> Result<()>;

    /// Remove the dasclaw CA from the trust store. Idempotent.
    fn uninstall(&self) -> Result<()>;

    /// Default `rotate` is `uninstall` followed by `install`. Platform
    /// modules MAY override to perform an atomic swap.
    fn rotate(&self, new_ca_pem: &[u8]) -> Result<()> {
        self.uninstall()?;
        self.install(new_ca_pem)
    }

    /// Snapshot of the current install state.
    fn status(&self) -> Result<CertStatus>;

    /// Return the PEM bytes of the installed CA, if available.
    fn export(&self) -> Result<Vec<u8>>;
}

/// Helper for stubs that are not yet implemented.
///
/// Only compiled when at least one stub backend (`linux` PR3 / generic
/// `fallback`) is selected. On macOS / Windows every entry point is
/// fully implemented so this helper is unreferenced and would warn as
/// dead code.
#[cfg(any(
    target_os = "linux",
    not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
))]
pub(crate) fn not_implemented<T>(operation: &'static str, platform: &'static str) -> Result<T> {
    Err(Error::NotImplemented {
        operation,
        platform,
    })
}

/// Return the active backend for the current target platform.
///
/// On unsupported platforms returns the env-var fallback backend, which
/// always reports `installed: false` and writes hints to `$CODEX_HOME/proxy/`
/// (PR1: stub returning `NotImplemented`).
pub fn current() -> Box<dyn TrustStore> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsTrustStore)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsTrustStore)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxTrustStore)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Box::new(fallback::FallbackTrustStore)
    }
}
