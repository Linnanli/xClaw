//! Type-safe network-deny reasons.
//!
//! Replaces the free-form `String` reasons used in `ironclaw-main` so that:
//!
//! - UX: callers can branch on reason category (allowlist miss vs invalid URL
//!   vs explicit kill-switch) for tailored messages.
//! - Automation: CI / SIEM / dashboards can aggregate by `kind()` without
//!   regex on log lines.
//! - SQL: stable enum discriminants survive log-rotation and i18n.
//!
//! `Display` is implemented to match the **exact** wording used by the
//! ironclaw-main port (W3.2b-1) so existing snapshot tests and downstream
//! log consumers keep working without churn.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Why a network request was denied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum NetworkDenyReason {
    /// The allowlist contains zero patterns — every request is denied by
    /// construction. This is typically a misconfiguration.
    EmptyAllowlist,

    /// The host is not covered by any allowlist pattern.
    HostNotAllowed {
        /// The denied host (already lowercased).
        host: String,
        /// Patterns that were checked, in declaration order.
        allowed: Vec<String>,
    },

    /// The proxy is operating in [`crate::ProxyMode::DenyAll`] (kill-switch).
    KillSwitch {
        /// Operator-supplied detail for audit logs.
        detail: String,
    },

    /// Request URL could not be parsed as `http://` or `https://`.
    InvalidUrl {
        /// The raw URL, for forensics. Already redacted of obvious creds by
        /// `url::Url::parse` (it strips `user:pass@` from the host string,
        /// but the original input is logged verbatim — callers must avoid
        /// passing fully-trusted secrets through URLs).
        url: String,
    },

    /// `CONNECT` request without an authority component.
    MissingHost,
}

impl NetworkDenyReason {
    /// Stable discriminant string (snake_case) for SQL/SIEM aggregation.
    /// Mirrors the serde tag so JSON-decoded events agree with this method.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::EmptyAllowlist => "empty_allowlist",
            Self::HostNotAllowed { .. } => "host_not_allowed",
            Self::KillSwitch { .. } => "kill_switch",
            Self::InvalidUrl { .. } => "invalid_url",
            Self::MissingHost => "missing_host",
        }
    }

    /// Convenience constructor for `HostNotAllowed`.
    pub fn host_not_allowed(host: impl Into<String>, allowed: Vec<String>) -> Self {
        Self::HostNotAllowed {
            host: host.into(),
            allowed,
        }
    }

    /// Convenience constructor for `KillSwitch`.
    pub fn kill_switch(detail: impl Into<String>) -> Self {
        Self::KillSwitch {
            detail: detail.into(),
        }
    }

    /// Convenience constructor for `InvalidUrl`.
    pub fn invalid_url(url: impl Into<String>) -> Self {
        Self::InvalidUrl { url: url.into() }
    }
}

impl fmt::Display for NetworkDenyReason {
    /// Human-readable rendering. Matches the wording produced by the
    /// ironclaw-main proxy in W3.2b-1, so log consumers remain compatible.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAllowlist => f.write_str("empty allowlist"),
            Self::HostNotAllowed { host, allowed } => {
                write!(
                    f,
                    "host '{}' not in allowlist: [{}]",
                    host,
                    allowed.join(", ")
                )
            }
            Self::KillSwitch { detail } => f.write_str(detail),
            Self::InvalidUrl { url } => write!(f, "Invalid URL: {}", url),
            Self::MissingHost => f.write_str("Missing host"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_strings_are_stable() {
        // These strings are part of the SIEM/SQL contract; never rename
        // without coordinating with downstream consumers.
        assert_eq!(NetworkDenyReason::EmptyAllowlist.kind(), "empty_allowlist");
        assert_eq!(
            NetworkDenyReason::host_not_allowed("a", vec![]).kind(),
            "host_not_allowed"
        );
        assert_eq!(NetworkDenyReason::kill_switch("k").kind(), "kill_switch");
        assert_eq!(NetworkDenyReason::invalid_url("u").kind(), "invalid_url");
        assert_eq!(NetworkDenyReason::MissingHost.kind(), "missing_host");
    }

    #[test]
    fn display_matches_ironclaw_main_wording() {
        // Backwards-compatible rendering — these literals appear in
        // log-search dashboards and operator runbooks.
        assert_eq!(
            NetworkDenyReason::EmptyAllowlist.to_string(),
            "empty allowlist"
        );
        assert_eq!(
            NetworkDenyReason::host_not_allowed(
                "evil.com",
                vec!["crates.io".to_string(), "*.github.com".to_string()],
            )
            .to_string(),
            "host 'evil.com' not in allowlist: [crates.io, *.github.com]"
        );
        assert_eq!(
            NetworkDenyReason::kill_switch("denied by policy").to_string(),
            "denied by policy"
        );
        assert_eq!(
            NetworkDenyReason::invalid_url("foo").to_string(),
            "Invalid URL: foo"
        );
        assert_eq!(NetworkDenyReason::MissingHost.to_string(), "Missing host");
    }
}
