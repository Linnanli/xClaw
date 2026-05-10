//! Cross-platform smoke tests for the public API surface.
//!
//! These verify that:
//! - every public function is reachable from a downstream crate,
//! - on Linux (PR3 not yet shipped) the stub still surfaces
//!   `NotImplemented` for the write paths.
//!
//! Per-platform lifecycle tests live in `tests/macos_integration.rs` /
//! `tests/windows_integration.rs` and are `#[ignore]`-gated since they
//! mutate the user's actual trust store. They isolate state via a
//! per-test `tempfile::TempDir` + `CODEX_HOME=…` so they never read or
//! clobber the developer's real `~/.codex/proxy/` directory.

use dasclaw_cert_trust::{export_ca_pem, install_ca, rotate_ca, status, uninstall_ca};

/// Compile-time check that the public symbols resolve from a downstream
/// crate (no runtime side-effects). If this stops linking, the API
/// surface has regressed.
#[allow(dead_code)]
fn _api_surface_links() {
    let _ = install_ca;
    let _ = uninstall_ca;
    let _ = rotate_ca;
    let _ = status;
    let _ = export_ca_pem;
}

#[test]
fn status_returns_a_value() {
    // We do not assert installed/uninstalled here — the developer's
    // `~/.codex/proxy/installed-ca.fingerprint` may legitimately exist
    // (and may even point to a real CA in the keychain). Just verify
    // the call itself does not error on supported platforms.
    let _ = status().expect("status must not error on supported platforms");
}

// ─── Linux still uses the PR1 stub until PR3 lands. ─────────────────
#[cfg(target_os = "linux")]
mod linux_stub {
    use dasclaw_cert_trust::{Error, install_ca, rotate_ca, uninstall_ca};

    const FAKE_PEM: &[u8] = b"-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";

    #[test]
    fn install_returns_not_implemented_on_linux() {
        let err = install_ca(FAKE_PEM).expect_err("Linux install is PR3");
        assert!(matches!(
            err,
            Error::NotImplemented {
                operation: "install",
                platform: "linux"
            }
        ));
    }

    #[test]
    fn uninstall_returns_not_implemented_on_linux() {
        let err = uninstall_ca().expect_err("Linux uninstall is PR3");
        assert!(matches!(
            err,
            Error::NotImplemented {
                operation: "uninstall",
                platform: "linux"
            }
        ));
    }

    #[test]
    fn rotate_returns_not_implemented_on_linux() {
        // Default rotate impl calls uninstall first → that stub fires.
        let err = rotate_ca(FAKE_PEM).expect_err("Linux rotate is PR3");
        assert!(matches!(err, Error::NotImplemented { .. }));
    }
}
