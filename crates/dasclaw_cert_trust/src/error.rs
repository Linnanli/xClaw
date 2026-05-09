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

    /// Underlying OS / I/O failure.
    #[error("dasclaw_cert_trust: I/O error: {0}")]
    Io(#[from] io::Error),
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
    fn io_error_converts_via_from() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "no such file");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}
