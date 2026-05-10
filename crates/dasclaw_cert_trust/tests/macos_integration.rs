//! macOS lifecycle tests — install → status → export → uninstall.
//!
//! These mutate the **actual** user keychain and trust settings, so they
//! are `#[ignore]`-gated and require an interactive GUI session
//! (`set_trust_settings_always` returns `errSecInternalComponent` over
//! SSH or in headless CI). Run locally with:
//!
//! ```bash
//! cargo nextest run -p dasclaw_cert_trust --run-ignored only --test macos_integration
//! ```
//!
//! State isolation: each test points `CODEX_HOME` at a fresh
//! [`tempfile::TempDir`] so the fingerprint sentinel never collides with
//! the developer's real `~/.codex/proxy/`.

#![cfg(target_os = "macos")]

use dasclaw_cert_trust::{export_ca_pem, install_ca, status, uninstall_ca};
use tempfile::TempDir;

const SAMPLE_CA_PEM: &[u8] = include_bytes!("fixtures/sample-ca.pem");

/// Set `CODEX_HOME` for the current process to a temp dir.
///
/// SAFETY: integration tests run one-per-process under nextest's default
/// process-per-test model, so no other thread is racing this env mutation.
fn isolate_codex_home() -> TempDir {
    let td = TempDir::new().expect("tempdir");
    // SAFETY: see module-level note about nextest process model.
    unsafe { std::env::set_var("CODEX_HOME", td.path()) };
    td
}

#[test]
#[ignore = "mutates real keychain; requires GUI session"]
fn install_status_export_uninstall_round_trip() {
    let _td = isolate_codex_home();
    install_ca(SAMPLE_CA_PEM).expect("install");
    let s = status().expect("status after install");
    assert!(s.installed, "status must show installed after install_ca");
    assert!(s.fingerprint_sha256.is_some());
    let pem = export_ca_pem().expect("export");
    assert!(pem.starts_with(b"-----BEGIN CERTIFICATE-----"));
    assert!(
        pem.windows(b"-----END CERTIFICATE-----".len())
            .any(|w| w == b"-----END CERTIFICATE-----"),
        "exported PEM must end with -----END CERTIFICATE-----"
    );
    uninstall_ca().expect("uninstall");
    let s = status().expect("status after uninstall");
    assert!(!s.installed, "uninstall must clear status");
}

#[test]
#[ignore = "mutates real keychain; requires GUI session"]
fn uninstall_is_idempotent_when_nothing_installed() {
    let _td = isolate_codex_home();
    // No install — uninstall should still be Ok.
    uninstall_ca().expect("first uninstall (no-op)");
    uninstall_ca().expect("second uninstall (no-op)");
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
