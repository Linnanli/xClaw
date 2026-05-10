//! Windows lifecycle tests — install → status → export → uninstall.
//!
//! These mutate the **actual** `CurrentUser\Root` certificate store, so
//! they are `#[ignore]`-gated. Adding a cert to the user root surfaces
//! a confirmation dialog the first time, so headless CI cannot run
//! these without the user clicking through. Run locally with:
//!
//! ```bash
//! cargo nextest run -p dasclaw_cert_trust --run-ignored only --test windows_integration
//! ```

#![cfg(target_os = "windows")]

use dasclaw_cert_trust::{export_ca_pem, install_ca, status, uninstall_ca};
use tempfile::TempDir;

const SAMPLE_CA_PEM: &[u8] = include_bytes!("fixtures/sample-ca.pem");

/// SAFETY: see analogous helper in `macos_integration.rs`.
fn isolate_codex_home() -> TempDir {
    let td = TempDir::new().expect("tempdir"); // safety: integration test helper, not production code
    // SAFETY: nextest runs one test per process by default.
    unsafe { std::env::set_var("CODEX_HOME", td.path()) };
    td
}

#[test]
#[ignore = "mutates CurrentUser\\Root; surfaces a confirmation dialog"]
fn install_status_export_uninstall_round_trip() {
    let _td = isolate_codex_home();
    install_ca(SAMPLE_CA_PEM).expect("install");
    let s = status().expect("status after install");
    assert!(s.installed);
    assert!(s.fingerprint_sha256.is_some());
    let pem = export_ca_pem().expect("export");
    assert!(pem.starts_with(b"-----BEGIN CERTIFICATE-----"));
    uninstall_ca().expect("uninstall");
    let s = status().expect("status after uninstall");
    assert!(!s.installed);
}

#[test]
#[ignore = "mutates CurrentUser\\Root"]
fn uninstall_is_idempotent_when_nothing_installed() {
    let _td = isolate_codex_home();
    uninstall_ca().expect("first uninstall");
    uninstall_ca().expect("second uninstall");
}

#[test]
fn install_rejects_malformed_pem() {
    let _td = isolate_codex_home();
    let err = install_ca(b"definitely not a pem").expect_err("must reject garbage");
    assert!(
        matches!(err, dasclaw_cert_trust::Error::InvalidPem(_)),
        "expected InvalidPem, got {err:?}"
    );
}

#[test]
fn install_rejects_empty_input() {
    let _td = isolate_codex_home();
    let err = install_ca(b"").expect_err("must reject empty");
    assert!(
        matches!(err, dasclaw_cert_trust::Error::InvalidPem(_)),
        "expected InvalidPem, got {err:?}"
    );
}

#[test]
fn export_without_install_returns_backend_error() {
    let _td = isolate_codex_home();
    let err = export_ca_pem().expect_err("export with no install must fail");
    assert!(
        matches!(err, dasclaw_cert_trust::Error::Backend { .. }),
        "expected Backend, got {err:?}"
    );
}
