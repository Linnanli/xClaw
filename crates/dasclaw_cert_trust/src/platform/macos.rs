//! macOS trust-store backend (per-user login keychain + per-user TLS
//! trust settings via Apple's `security-framework`).
//!
//! Implementation strategy (ADR-139 §4.5 PR2):
//!
//! 1. **install** — decode the PEM into DER, build a `SecCertificate`,
//!    add it to the user's default (login) keychain, then call
//!    `TrustSettings::set_trust_settings_always` in the `Domain::User`
//!    domain so the kernel TLS evaluator treats it as a root.
//!
//! 2. **status** — re-read the SHA-256 fingerprint sentinel written at
//!    install time, walk the user-domain trust settings looking for a
//!    matching DER blob. The sentinel is the single source of truth for
//!    "is dasclaw's CA installed?"; without it we refuse to claim
//!    ownership of any random root the user may have added themselves.
//!
//! 3. **export** — reuse `find_cert_by_fingerprint` to locate the cert,
//!    then PEM-encode the DER bytes returned by `SecCertificate::to_der()`.
//!
//! 4. **uninstall** — locate the cert via the fingerprint, call
//!    `SecCertificate::delete()` (= `SecItemDelete`) to drop it from the
//!    keychain. The user-domain trust setting is removed automatically
//!    when the cert disappears. Idempotent: a missing sentinel or
//!    missing cert both yield `Ok(())`.
//!
//! # GUI session requirement
//!
//! `set_trust_settings_always` returns `errSecInternalComponent (-2070)`
//! when invoked outside an interactive session (per Apple's
//! `SecTrustSettings` docs and confirmed in `security-framework`'s own
//! source comment). On CI runners without a GUI the integration test in
//! `tests/macos_integration.rs` is gated with `#[ignore]`; developers
//! must run `cargo nextest run -p dasclaw_cert_trust --run-ignored only`
//! locally to exercise the full lifecycle.

use security_framework::base::Error as SfError;
use security_framework::certificate::SecCertificate;
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::trust_settings::{Domain, TrustSettings};

use super::TrustStore;
use crate::Result;
use crate::error::Error;
use crate::paths;
use crate::pem;
use crate::status::CertStatus;

const PLATFORM: &str = "macos";

// `errSec*` constants — `security-framework` does not re-export these
// publicly, so duplicate the few we branch on. Values are stable per
// Apple's `<Security/SecBase.h>`.
const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SEC_USER_CANCELED: i32 = -128;
const ERR_SEC_AUTH_FAILED: i32 = -25293;
const ERR_SEC_INTERNAL_COMPONENT: i32 = -2070;

/// Backend selected on `cfg(target_os = "macos")`.
pub struct MacOsTrustStore;

impl TrustStore for MacOsTrustStore {
    fn platform(&self) -> &'static str {
        PLATFORM
    }

    fn install(&self, ca_pem: &[u8]) -> Result<()> {
        let der = pem::decode_first_certificate(ca_pem)?;
        let fingerprint = pem::sha256_hex(&der);
        let cert =
            SecCertificate::from_der(&der).map_err(|e| map_sf_err("install (parse DER)", e))?;
        // Adding to the login keychain may surface a "this app wants to
        // modify your keychain" prompt the first time. `errSecDuplicateItem`
        // is benign — proceed to (re-)apply trust settings.
        let keychain = SecKeychain::default().map_err(|e| map_sf_err("install (keychain)", e))?;
        match cert.add_to_keychain(Some(keychain)) {
            Ok(()) => {}
            Err(e) if e.code() == ERR_SEC_DUPLICATE_ITEM => {}
            Err(e) => return Err(map_sf_err("install (add cert)", e)),
        }
        TrustSettings::new(Domain::User)
            .set_trust_settings_always(&cert)
            .map_err(|e| map_sf_err("install (set trust)", e))?;
        paths::write_fingerprint(&fingerprint)?;
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            // No record of an install — treat as already-uninstalled.
            return Ok(());
        };
        if let Some(cert) = find_cert_by_fingerprint(&target_fp)? {
            match cert.delete() {
                Ok(()) => {}
                Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {}
                Err(e) => return Err(map_sf_err("uninstall (delete)", e)),
            }
        }
        // Either we deleted, or the cert was already gone — clear the
        // sentinel either way so subsequent calls converge to "uninstalled".
        paths::clear_fingerprint()?;
        Ok(())
    }

    fn status(&self) -> Result<CertStatus> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            return Ok(CertStatus::not_installed(PLATFORM));
        };
        match find_cert_by_fingerprint(&target_fp)? {
            Some(_cert) => Ok(CertStatus {
                installed: true,
                fingerprint_sha256: Some(target_fp),
                expires_at: None, // PR2 does not parse X.509 — see ADR-139 follow-up.
                platform: PLATFORM,
                env_fallback_active: false,
            }),
            None => Ok(CertStatus::not_installed(PLATFORM)),
        }
    }

    fn export(&self) -> Result<Vec<u8>> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            return Err(Error::Backend {
                operation: "export",
                reason: "no CA recorded — run `dasclaw cert install` first".into(),
            });
        };
        let Some(cert) = find_cert_by_fingerprint(&target_fp)? else {
            return Err(Error::Backend {
                operation: "export",
                reason: format!("CA fingerprint {target_fp} not found in user trust settings"),
            });
        };
        Ok(der_to_pem(&cert.to_der()))
    }
}

/// Walk `Domain::User` trust settings for a certificate whose DER body
/// hashes to `fp_hex`. Returns the matching cert or `None`.
fn find_cert_by_fingerprint(fp_hex: &str) -> Result<Option<SecCertificate>> {
    let iter = TrustSettings::new(Domain::User)
        .iter()
        .map_err(|e| map_sf_err("status (iter trust settings)", e))?;
    for cert in iter {
        let der = cert.to_der();
        if pem::sha256_hex(&der) == fp_hex {
            return Ok(Some(cert));
        }
    }
    Ok(None)
}

/// Wrap a `SecBase` error into [`Error`], classifying user-cancel and
/// auth-failure as `PermissionDenied` (fail-safe per ADR-139 §3.7).
fn map_sf_err(operation: &'static str, err: SfError) -> Error {
    let code = err.code();
    if code == ERR_SEC_USER_CANCELED
        || code == ERR_SEC_AUTH_FAILED
        || code == ERR_SEC_INTERNAL_COMPONENT
    {
        return Error::PermissionDenied;
    }
    let msg = err.message().unwrap_or_else(|| format!("OSStatus {code}"));
    Error::Backend {
        operation,
        reason: format!("{msg} (code {code})"),
    }
}

/// PEM-wrap a DER blob with standard 64-char line wrapping.
fn der_to_pem(der: &[u8]) -> Vec<u8> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::with_capacity(b64.len() + 64);
    out.push_str("-----BEGIN CERTIFICATE-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        // `b64` is pure ASCII, so every chunk is valid UTF-8.
        match std::str::from_utf8(chunk) {
            Ok(line) => out.push_str(line),
            Err(_) => out.push_str(""),
        }
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out.into_bytes()
}
