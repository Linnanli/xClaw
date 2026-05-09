//! Smoke tests for the public scaffold API.
//!
//! These verify PR1 contracts only: every operation is callable, returns
//! the expected `NotImplemented` shape (or a successful "not installed"
//! status), and the platform name flows through correctly. PR2/PR3 will
//! add real-backend integration tests.

use dasclaw_cert_trust::{Error, install_ca, rotate_ca, status, uninstall_ca};

const FAKE_PEM: &[u8] = b"-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";

#[test]
fn install_returns_not_implemented_on_current_platform() {
    let err = install_ca(FAKE_PEM).expect_err("install must be a stub in PR1");
    match err {
        Error::NotImplemented {
            operation,
            platform,
        } => {
            assert_eq!(operation, "install");
            assert!(!platform.is_empty(), "platform must be non-empty");
        }
        other => panic!("expected NotImplemented, got {other:?}"),
    }
}

#[test]
fn uninstall_returns_not_implemented() {
    let err = uninstall_ca().expect_err("uninstall must be a stub in PR1");
    assert!(matches!(
        err,
        Error::NotImplemented {
            operation: "uninstall",
            ..
        }
    ));
}

#[test]
fn rotate_uses_default_uninstall_then_install_path() {
    // Default trait impl tries `uninstall` first → that stub fires first.
    let err = rotate_ca(FAKE_PEM).expect_err("rotate must be a stub in PR1");
    assert!(matches!(err, Error::NotImplemented { .. }));
}

#[test]
fn status_returns_not_installed_on_supported_platforms() {
    let s = status().expect("status must succeed even when stubbed");
    assert!(!s.installed);
    assert!(s.fingerprint_sha256.is_none());
    assert!(!s.platform.is_empty());
}
