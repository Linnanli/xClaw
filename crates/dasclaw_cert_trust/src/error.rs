//! Error types for `dasclaw_cert_trust`.

use std::io;

/// Errors returned by the trust-chain manager.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The requested operation is not yet implemented for this platform.
    ///
    /// PR1 (ADR-139 §4.5) ships only API + skeleton; PR2/PR3 replace these.
    #[error("dasclaw_cert_trust: {operation} not yet implemented on {platform}")]
    NotImplemented {
        /// The operation that was attempted (e.g. `"install"`).
        operation: &'static str,
        /// The current platform name (e.g. `"macos"`, `"windows"`, `"linux"`).
        platform: &'static str,
    },

    /// The user denied a keychain prompt or lacks the required permission.
    #[error("dasclaw_cert_trust: permission denied accessing trust store")]
    PermissionDenied,

    /// The CA bytes could not be parsed as a PEM certificate.
    #[error("dasclaw_cert_trust: invalid CA PEM: {0}")]
    InvalidPem(String),

    /// The PEM decoded successfully but the X.509 body is unfit to be
    /// installed as a per-user root CA.
    ///
    /// Examples (issue #375 / ADR-139 §3.7 fail-safe):
    /// - `BasicConstraints` extension absent or `cA: false` (leaf cert)
    /// - `notAfter` already in the past (expired CA)
    /// - issuer ≠ subject (non-self-signed intermediate)
    ///
    /// Distinguished from [`Error::InvalidPem`] (which means the input
    /// was not even syntactically a certificate) so the CLI can give a
    /// more actionable message.
    #[error("dasclaw_cert_trust: CA cert rejected: {0}")]
    InvalidCa(String),

    /// Underlying OS / I/O failure.
    #[error("dasclaw_cert_trust: I/O error: {0}")]
    Io(#[from] io::Error),

    /// Platform trust-store backend rejected the operation for a reason
    /// that is neither `PermissionDenied` nor an I/O error.
    ///
    /// Examples: macOS Security framework returned `errSecDuplicateItem`;
    /// Windows `CertOpenStore` returned `ERROR_FILE_NOT_FOUND`. The string
    /// payload captures the OS-specific error code/message verbatim.
    #[error("dasclaw_cert_trust: {operation} failed: {reason}")]
    Backend {
        /// What we were trying to do (e.g. `"install"`, `"uninstall"`).
        operation: &'static str,
        /// Free-form OS error description (already redacted of secrets).
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_implemented_display_includes_operation_and_platform() {
        let err = Error::NotImplemented {
            operation: "install",
            platform: "macos",
        };
        let msg = err.to_string();
        assert!(msg.contains("install"), "missing operation: {msg}");
        assert!(msg.contains("macos"), "missing platform: {msg}");
    }

    #[test]
    fn invalid_pem_includes_reason() {
        let err = Error::InvalidPem("missing -----BEGIN CERTIFICATE-----".into());
        assert!(err.to_string().contains("missing -----BEGIN"));
    }

    #[test]
    fn invalid_ca_includes_reason() {
        let err = Error::InvalidCa("BasicConstraints CA:FALSE".into());
        let msg = err.to_string();
        assert!(msg.contains("CA cert rejected"), "wrong prefix: {msg}");
        assert!(msg.contains("BasicConstraints"), "missing reason: {msg}");
    }

    #[test]
    fn io_error_converts_via_from() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "no such file");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn backend_display_includes_operation_and_reason() {
        let err = Error::Backend {
            operation: "install",
            reason: "errSecDuplicateItem (-25299)".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("install"), "missing operation: {msg}");
        assert!(msg.contains("errSecDuplicateItem"), "missing reason: {msg}");
    }
}
