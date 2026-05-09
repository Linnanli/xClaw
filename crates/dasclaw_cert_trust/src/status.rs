//! Reported state of the dasclaw MITM CA in the user's trust store.

use serde::{Deserialize, Serialize};

/// Snapshot of the install state used by `dasclaw cert status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertStatus {
    /// `true` when the CA is registered with the OS trust store.
    pub installed: bool,
    /// SHA-256 fingerprint of the installed CA, if any (lowercase hex).
    pub fingerprint_sha256: Option<String>,
    /// CA expiry (RFC 3339 timestamp), if known.
    pub expires_at: Option<String>,
    /// Human-readable platform name (e.g. `"macos"`, `"windows"`, `"linux"`).
    pub platform: &'static str,
    /// Whether the env-var fallback (`SSL_CERT_FILE` etc.) is active.
    pub env_fallback_active: bool,
}

impl CertStatus {
    /// Build a "no CA installed" status for a given platform.
    pub fn not_installed(platform: &'static str) -> Self {
        Self {
            installed: false,
            fingerprint_sha256: None,
            expires_at: None,
            platform,
            env_fallback_active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_constructor_sets_platform() {
        let s = CertStatus::not_installed("macos");
        assert!(!s.installed);
        assert_eq!(s.platform, "macos");
        assert!(s.fingerprint_sha256.is_none());
        assert!(!s.env_fallback_active);
    }

    #[test]
    fn installed_status_carries_fingerprint() {
        let s = CertStatus {
            installed: true,
            fingerprint_sha256: Some("abc123".into()),
            expires_at: Some("2027-01-01T00:00:00Z".into()),
            platform: "linux",
            env_fallback_active: true,
        };
        assert!(s.installed);
        assert_eq!(s.fingerprint_sha256.as_deref(), Some("abc123"));
        assert_eq!(s.platform, "linux");
    }
}
