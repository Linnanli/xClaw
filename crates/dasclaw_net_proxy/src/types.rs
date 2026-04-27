//! Credential injection types (decoupled from secrets backend).
//!
//! These mirror `ironclaw-main/src/secrets/types.rs` but contain only the
//! pure-data structures needed by the proxy's policy engine. The actual
//! credential **resolution** (looking up secret values) is delegated to
//! the `CredentialResolver` trait so this crate has no dependency on any
//! secrets-store implementation.

use serde::{Deserialize, Serialize};

/// Where a credential should be injected into an HTTP request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CredentialLocation {
    /// `Authorization: Bearer {secret}`.
    #[default]
    AuthorizationBearer,
    /// `Authorization: Basic base64(username:secret)`.
    ///
    /// Note: forward proxy implementation currently logs and skips this
    /// location (requires base64-encoding a username:secret pair); tools
    /// needing Basic auth should fetch credentials out-of-band.
    AuthorizationBasic { username: String },
    /// Custom header injection.
    Header {
        name: String,
        prefix: Option<String>,
    },
    /// Query parameter injection.
    QueryParam { name: String },
    /// URL placeholder substitution.
    ///
    /// Note: forward proxy implementation currently logs and skips this
    /// location (requires rewriting the request URI).
    UrlPath { placeholder: String },
}

/// Mapping from a secret name to where/when it should be injected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMapping {
    /// Name of the secret to resolve.
    pub secret_name: String,
    /// Where to inject the resolved value.
    pub location: CredentialLocation,
    /// Host glob patterns this credential applies to (e.g. `*.openai.com`).
    pub host_patterns: Vec<String>,
    /// When `true`, the request proceeds even if the secret cannot be resolved.
    ///
    /// **Defaults to `false` (required)** so a tool that simply declares a
    /// credential without explicitly opting into "optional" cannot be
    /// silently downgraded to an unauthenticated request.
    #[serde(default)]
    pub optional: bool,
}

impl CredentialMapping {
    /// Convenience constructor for a Bearer-token mapping.
    pub fn bearer(secret_name: impl Into<String>, host_pattern: impl Into<String>) -> Self {
        Self {
            secret_name: secret_name.into(),
            location: CredentialLocation::AuthorizationBearer,
            host_patterns: vec![host_pattern.into()],
            optional: false,
        }
    }

    /// Convenience constructor for a custom-header mapping.
    pub fn header(
        secret_name: impl Into<String>,
        header_name: impl Into<String>,
        host_pattern: impl Into<String>,
    ) -> Self {
        Self {
            secret_name: secret_name.into(),
            location: CredentialLocation::Header {
                name: header_name.into(),
                prefix: None,
            },
            host_patterns: vec![host_pattern.into()],
            optional: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_default_optional_false() {
        let m = CredentialMapping::bearer("OPENAI_API_KEY", "api.openai.com");
        assert!(!m.optional, "Fail-Safe default: required, not optional");
        assert_eq!(m.secret_name, "OPENAI_API_KEY");
        assert!(matches!(
            m.location,
            CredentialLocation::AuthorizationBearer
        ));
    }

    #[test]
    fn header_with_no_prefix() {
        let m = CredentialMapping::header("X_KEY", "X-Api-Key", "api.example.com");
        match &m.location {
            CredentialLocation::Header { name, prefix } => {
                assert_eq!(name, "X-Api-Key");
                assert!(prefix.is_none());
            }
            _ => panic!("expected Header"),
        }
    }
}
