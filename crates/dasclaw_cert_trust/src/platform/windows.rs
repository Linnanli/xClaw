//! Windows trust-store backend (`CurrentUser\Root` system store via
//! `windows-rs` Crypt32 bindings).
//!
//! Implementation strategy (ADR-139 §4.5 PR2):
//!
//! 1. **install** — decode PEM → DER, open `CurrentUser\Root` with
//!    [`CertOpenStore`], call [`CertAddEncodedCertificateToStore`] with
//!    `CERT_STORE_ADD_REPLACE_EXISTING` so re-running `install` after a
//!    rotation cleanly overwrites the previous cert. Persist the
//!    SHA-256 fingerprint sentinel afterward.
//!
//! 2. **status / export / uninstall** — locate the dasclaw cert by
//!    iterating [`CertEnumCertificatesInStore`] and matching DER bytes
//!    against the fingerprint sentinel. `uninstall` then calls
//!    [`CertDeleteCertificateFromStore`].
//!
//! `CurrentUser\Root` does **not** require admin privileges (unlike
//! `LocalMachine\Root`) — Windows still surfaces a confirmation dialog
//! the first time a certificate is added to the user's root store, so
//! the test in `tests/windows_integration.rs` is `#[ignore]`-gated.
//!
//! All FFI calls are `unsafe` per windows-rs's contract; the unsafe
//! blocks are kept narrow and each is annotated with the invariants we
//! rely on.
//!
//! [`CertOpenStore`]: windows::Win32::Security::Cryptography::CertOpenStore

use windows::Win32::Security::Cryptography::{
    CERT_CONTEXT, CERT_QUERY_ENCODING_TYPE, CERT_STORE_ADD_REPLACE_EXISTING,
    CERT_STORE_PROV_SYSTEM_W, CERT_SYSTEM_STORE_CURRENT_USER, CertAddEncodedCertificateToStore,
    CertCloseStore, CertDeleteCertificateFromStore, CertDuplicateCertificateContext,
    CertEnumCertificatesInStore, CertOpenStore, HCERTSTORE, X509_ASN_ENCODING,
};
use windows::core::Error as WinError;

use super::TrustStore;
use crate::Result;
use crate::error::Error;
use crate::paths;
use crate::pem;
use crate::status::CertStatus;

const PLATFORM: &str = "windows";

/// UTF-16 NUL-terminated literal `"ROOT"` — the Windows system store
/// containing trusted root CAs. Built once at module init time.
fn store_name_root() -> Vec<u16> {
    "ROOT\0".encode_utf16().collect()
}

/// Backend selected on `cfg(target_os = "windows")`.
pub struct WindowsTrustStore;

impl TrustStore for WindowsTrustStore {
    fn platform(&self) -> &'static str {
        PLATFORM
    }

    fn install(&self, ca_pem: &[u8]) -> Result<()> {
        let der = pem::decode_and_validate_root_ca(ca_pem)?;
        let fingerprint = pem::sha256_hex(&der);
        let store = open_root_store()?;
        let result = unsafe {
            // SAFETY: `store` is a valid HCERTSTORE returned by
            // CertOpenStore above; `der` outlives this call.
            CertAddEncodedCertificateToStore(
                store.handle(),
                CERT_QUERY_ENCODING_TYPE(X509_ASN_ENCODING.0),
                &der,
                CERT_STORE_ADD_REPLACE_EXISTING,
                None,
            )
        };
        // `store` drops here, closing the handle automatically.
        drop(store);
        result.map_err(|e| map_win_err("install (add cert)", e))?;
        paths::write_fingerprint(&fingerprint)?;
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            return Ok(());
        };
        let store = open_root_store()?;
        if let Some(ctx) = find_cert_in_store(store.handle(), &target_fp)? {
            // SAFETY: `ctx` is a freshly-duplicated context owned by us;
            // `CertDeleteCertificateFromStore` consumes (frees) it.
            unsafe {
                CertDeleteCertificateFromStore(ctx)
                    .map_err(|e| map_win_err("uninstall (delete)", e))?;
            }
        }
        drop(store);
        paths::clear_fingerprint()?;
        Ok(())
    }

    fn status(&self) -> Result<CertStatus> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            return Ok(CertStatus::not_installed(PLATFORM));
        };
        let store = open_root_store()?;
        let found = find_cert_in_store(store.handle(), &target_fp)?;
        // Extract `notAfter` from the encoded DER while we still own
        // the context, then free it. Issue #376 §3.4 — graceful
        // degradation: parse failures yield `expires_at: None` rather
        // than masking `installed: true`.
        let (installed, expires_at) = match found {
            Some(ctx) => {
                // SAFETY: `ctx` came from CertDuplicateCertificateContext;
                // `pbCertEncoded` is valid for `cbCertEncoded` bytes for
                // the lifetime of the context.
                let expires_at = unsafe {
                    let info = &*ctx;
                    let der =
                        std::slice::from_raw_parts(info.pbCertEncoded, info.cbCertEncoded as usize);
                    pem::extract_not_after_rfc3339(der)
                };
                free_dup_context(ctx);
                (true, expires_at)
            }
            None => (false, None),
        };
        drop(store);
        if installed {
            Ok(CertStatus {
                installed: true,
                fingerprint_sha256: Some(target_fp),
                expires_at,
                platform: PLATFORM,
                env_fallback_active: false,
            })
        } else {
            Ok(CertStatus::not_installed(PLATFORM))
        }
    }

    fn export(&self) -> Result<Vec<u8>> {
        let Some(target_fp) = paths::read_fingerprint()? else {
            return Err(Error::Backend {
                operation: "export",
                reason: "no CA recorded — run `dasclaw cert install` first".into(),
            });
        };
        let store = open_root_store()?;
        let Some(ctx) = find_cert_in_store(store.handle(), &target_fp)? else {
            drop(store);
            return Err(Error::Backend {
                operation: "export",
                reason: format!("CA fingerprint {target_fp} not found in CurrentUser\\Root"),
            });
        };
        // SAFETY: `ctx` points to a valid CERT_CONTEXT we own; the
        // `pbCertEncoded` buffer is valid for `cbCertEncoded` bytes for
        // the lifetime of the context.
        let der = unsafe {
            let info = &*ctx;
            std::slice::from_raw_parts(info.pbCertEncoded, info.cbCertEncoded as usize).to_vec()
        };
        free_dup_context(ctx);
        drop(store);
        Ok(pem::der_to_pem(&der))
    }
}

/// RAII wrapper that closes an HCERTSTORE on drop.
struct CertStoreHandle(HCERTSTORE);

impl CertStoreHandle {
    fn handle(&self) -> HCERTSTORE {
        self.0
    }
}

impl Drop for CertStoreHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` is a valid HCERTSTORE returned by
            // `CertOpenStore`; we hand ownership back here exactly once.
            unsafe {
                let _ = CertCloseStore(self.0, 0);
            }
        }
    }
}

fn open_root_store() -> Result<CertStoreHandle> {
    let name = store_name_root();
    // SAFETY: `CERT_STORE_PROV_SYSTEM_W` accepts a UTF-16 store name in
    // `pvPara`; `name` is NUL-terminated and outlives the call.
    let store = unsafe {
        CertOpenStore(
            CERT_STORE_PROV_SYSTEM_W,
            CERT_QUERY_ENCODING_TYPE(0),
            None,
            windows::Win32::Security::Cryptography::CERT_OPEN_STORE_FLAGS(
                CERT_SYSTEM_STORE_CURRENT_USER,
            ),
            Some(name.as_ptr() as *const _),
        )
    }
    .map_err(|e| map_win_err("open store", e))?;
    if store.is_invalid() {
        return Err(Error::Backend {
            operation: "open store",
            reason: "CertOpenStore returned NULL handle".into(),
        });
    }
    Ok(CertStoreHandle(store))
}

/// Walk every certificate in `store`, returning a duplicated handle to
/// the first one whose DER body hashes to `fp_hex`. Caller must free
/// the returned context with [`free_dup_context`].
fn find_cert_in_store(store: HCERTSTORE, fp_hex: &str) -> Result<Option<*mut CERT_CONTEXT>> {
    let mut cursor: *const CERT_CONTEXT = std::ptr::null();
    loop {
        // SAFETY: passing the previous `cursor` advances enumeration;
        // a NULL return ends the walk per Crypt32 docs.
        cursor = unsafe { CertEnumCertificatesInStore(store, Some(cursor)) };
        if cursor.is_null() {
            break;
        }
        // SAFETY: when non-null, `cursor` points to a CERT_CONTEXT
        // owned by `store`; pbCertEncoded is valid for cbCertEncoded
        // bytes while we hold the cursor.
        let der = unsafe {
            let info = &*cursor;
            std::slice::from_raw_parts(info.pbCertEncoded, info.cbCertEncoded as usize)
        };
        if pem::sha256_hex(der) == fp_hex {
            // Duplicate so we can return the handle past the next
            // CertEnumCertificatesInStore call (which would free the
            // store-owned cursor).
            // SAFETY: `cursor` is a valid CERT_CONTEXT*.
            let dup = unsafe { CertDuplicateCertificateContext(Some(cursor)) };
            if dup.is_null() {
                return Err(Error::Backend {
                    operation: "find cert",
                    reason: "CertDuplicateCertificateContext returned NULL".into(),
                });
            }
            return Ok(Some(dup));
        }
    }
    Ok(None)
}

/// Free a context obtained from [`CertDuplicateCertificateContext`].
fn free_dup_context(ctx: *mut CERT_CONTEXT) {
    // `CertFreeCertificateContext` takes `Option<*const CERT_CONTEXT>`
    // and decrements the refcount. We obtained `ctx` via Duplicate, so
    // we own one refcount.
    use windows::Win32::Security::Cryptography::CertFreeCertificateContext;
    // SAFETY: `ctx` was obtained from CertDuplicateCertificateContext
    // and has not been freed.
    unsafe {
        let _ = CertFreeCertificateContext(Some(ctx));
    }
}

/// Wrap a `windows::core::Error` into [`Error`].
///
/// Windows surfaces "user denied" UAC / cert dialogs as
/// `ERROR_CANCELLED` (HRESULT 0x800704C7); we map that to
/// `PermissionDenied` per ADR-139 §3.7 fail-safe.
fn map_win_err(operation: &'static str, err: WinError) -> Error {
    const ERROR_CANCELLED_HRESULT: i32 = 0x800704C7u32 as i32;
    if err.code().0 == ERROR_CANCELLED_HRESULT {
        return Error::PermissionDenied;
    }
    Error::Backend {
        operation,
        reason: format!("{err} (HRESULT {:#010x})", err.code().0 as u32),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure-function pieces (lifecycle is covered by
    //! `tests/windows_integration.rs`). Locks the ADR-139 §3.7 fail-safe
    //! error classification — `ERROR_CANCELLED` from a UAC / root-store
    //! confirmation dialog must surface as `PermissionDenied`, not as a
    //! generic `Backend` error.

    use super::*;
    use windows::core::HRESULT;

    #[test]
    fn user_cancel_maps_to_permission_denied() {
        let err = WinError::from(HRESULT(0x800704C7u32 as i32));
        let mapped = map_win_err("install", err);
        assert!(
            matches!(mapped, Error::PermissionDenied),
            "user-cancel must fail-safe: {mapped:?}"
        );
    }

    #[test]
    fn other_hresult_maps_to_backend_with_operation() {
        // ERROR_FILE_NOT_FOUND (0x80070002) — a non-cancel failure must
        // preserve the operation tag so callers can localize the error.
        let err = WinError::from(HRESULT(0x80070002u32 as i32));
        let mapped = map_win_err("uninstall (delete)", err);
        match mapped {
            Error::Backend { operation, reason } => {
                assert_eq!(operation, "uninstall (delete)");
                assert!(
                    reason.contains("0x80070002"),
                    "reason must include HRESULT: {reason}"
                );
            }
            other => panic!("expected Backend, got {other:?}"),
        }
    }
}
