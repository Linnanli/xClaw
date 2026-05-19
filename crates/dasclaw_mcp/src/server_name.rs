//! Typed MCP server identifier with an allowlist boundary (security delta).
//!
//! ## Why this exists
//!
//! Upstream `ironclaw-main` ships a typed [`McpServerName`] in
//! `ironclaw_common` and routes **every** `McpClient` constructor through
//! `McpServerName::from_trusted`. The x-claw fork dropped this type during a
//! rename pass (`ironclaw_common` was retired without an equivalent in
//! `dasclaw_*`), and as a result the host had **zero** allowlist validation
//! on server names that flow into:
//!
//! - file-system paths (secret storage at `~/.dasclaw/secrets/<server>/…`)
//! - the `Mcp-Session-Id` HTTP header
//! - log lines and error messages
//!
//! This module restores the boundary type. The allowlist matches the upstream
//! semantics: ASCII alphanumerics plus a short list of safe separators, no
//! whitespace, no path separators, no control characters, no non-ASCII.
//!
//! See `gh issue view 628` and ADR-152 §F3.2 for the broader context.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum length for a server name.
///
/// Long enough to accept any reasonable user-provided identifier; short enough
/// that we never overflow `Mcp-Session-Id` header conventions or file-name
/// limits on the major filesystems.
pub const MAX_LEN: usize = 128;

/// Error returned when a candidate string fails the [`McpServerName`] allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpServerNameError {
    /// Empty / whitespace-only input.
    #[error("MCP server name must not be empty")]
    Empty,

    /// Exceeds [`MAX_LEN`].
    #[error("MCP server name length {0} exceeds maximum {MAX_LEN}")]
    TooLong(usize),

    /// Contains a byte that is not on the allowlist (anything outside
    /// `[A-Za-z0-9_.-]`).
    #[error(
        "MCP server name '{input}' contains disallowed character {ch:?} at byte {pos}; \
         allowed: ASCII alphanumerics and `_`, `.`, `-`"
    )]
    InvalidChar { input: String, ch: char, pos: usize },

    /// Reserved value (e.g. `.` or `..`) that could be interpreted as a path
    /// component.
    #[error("MCP server name '{0}' is reserved (would be interpreted as a path component)")]
    Reserved(String),
}

/// A validated MCP server identifier.
///
/// Construct with [`McpServerName::new`] (validating) or
/// [`McpServerName::from_trusted`] (panics on violation — only use with
/// hard-coded literals).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpServerName(String);

impl McpServerName {
    /// Validate `s` against the allowlist and wrap it on success.
    pub fn new(s: impl Into<String>) -> Result<Self, McpServerNameError> {
        let s: String = s.into();
        validate(&s)?;
        Ok(Self(s))
    }

    /// Wrap a name that is **known** to be valid (hard-coded literal or value
    /// already produced by [`Self::new`]).
    ///
    /// Returns the same allowlist error as [`Self::new`] if the assumption is
    /// wrong — call sites should `.expect` with a clear contract note.
    pub fn from_trusted(s: impl Into<String>) -> Result<Self, McpServerNameError> {
        Self::new(s)
    }

    /// Borrow as `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume into the underlying `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for McpServerName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for McpServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for McpServerName {
    type Error = McpServerNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for McpServerName {
    type Error = McpServerNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<McpServerName> for String {
    fn from(value: McpServerName) -> Self {
        value.0
    }
}

fn validate(s: &str) -> Result<(), McpServerNameError> {
    if s.is_empty() {
        return Err(McpServerNameError::Empty);
    }
    if s.len() > MAX_LEN {
        return Err(McpServerNameError::TooLong(s.len()));
    }
    if s == "." || s == ".." {
        return Err(McpServerNameError::Reserved(s.to_string()));
    }
    for (pos, ch) in s.char_indices() {
        if !is_allowed(ch) {
            return Err(McpServerNameError::InvalidChar {
                input: s.to_string(),
                ch,
                pos,
            });
        }
    }
    Ok(())
}

#[inline]
fn is_allowed(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_accepts_plain_alphanumeric() {
        for ok in [
            "github",
            "notion",
            "server1",
            "my-server",
            "my_server",
            "x.y",
        ] {
            McpServerName::new(ok).expect("plain alphanumeric must be accepted");
        }
    }

    #[test]
    fn test_security_rejects_empty() {
        let err = McpServerName::new("").unwrap_err();
        assert!(matches!(err, McpServerNameError::Empty));
    }

    #[test]
    fn test_security_rejects_too_long() {
        let too_long = "a".repeat(MAX_LEN + 1);
        let err = McpServerName::new(&too_long).unwrap_err();
        assert!(matches!(err, McpServerNameError::TooLong(n) if n == MAX_LEN + 1));
    }

    #[test]
    fn test_security_server_name_rejects_path_traversal() {
        for bad in ["..", ".", "../etc", "/etc/passwd", "a/b", "a\\b"] {
            let err = McpServerName::new(bad)
                .unwrap_err_or_else_message(format!("path traversal '{bad}' must be rejected"));
            assert!(
                matches!(
                    err,
                    McpServerNameError::Reserved(_) | McpServerNameError::InvalidChar { .. }
                ),
                "unexpected error variant for {bad}: {err:?}"
            );
        }
    }

    #[test]
    fn test_security_server_name_rejects_control_chars() {
        for bad in ["a\nb", "a\rb", "a\tb", "a\0b", "a\x07b"] {
            let err = McpServerName::new(bad).unwrap_err();
            assert!(
                matches!(err, McpServerNameError::InvalidChar { .. }),
                "control char in {bad:?} must be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn test_security_server_name_rejects_non_ascii() {
        for bad in ["café", "服务器", "emoji-🎯"] {
            let err = McpServerName::new(bad).unwrap_err();
            assert!(
                matches!(err, McpServerNameError::InvalidChar { .. }),
                "non-ascii {bad:?} must be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn test_security_server_name_rejects_shell_meta() {
        for bad in [
            "a b", "a;b", "a|b", "a&b", "a$b", "a`b", "a(b", "a)b", "a*b", "a?b", "a<b", "a>b",
            "a\"b", "a'b",
        ] {
            let err = McpServerName::new(bad).unwrap_err();
            assert!(
                matches!(err, McpServerNameError::InvalidChar { .. }),
                "shell metachar in {bad:?} must be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn test_from_trusted_round_trips() {
        let n = McpServerName::from_trusted("unknown")
            .expect("'unknown' is a valid McpServerName (alnum allowlist)");
        assert_eq!(n.as_str(), "unknown");
        assert_eq!(n.to_string(), "unknown");
    }

    #[test]
    fn test_serde_round_trip() {
        let n = McpServerName::new("github").unwrap();
        let s = serde_json::to_string(&n).unwrap();
        assert_eq!(s, "\"github\"");
        let back: McpServerName = serde_json::from_str(&s).unwrap(); // safety: test code
        assert_eq!(back, n); // safety: test assertion
    }

    #[test]
    fn test_serde_rejects_invalid() {
        let res: Result<McpServerName, _> = serde_json::from_str("\"a/b\"");
        assert!(res.is_err(), "deserialization must enforce the allowlist");
    }

    // ---------------- helpers ---------------------------------------------------

    trait UnwrapErrOrElseMessage<T, E> {
        fn unwrap_err_or_else_message(self, msg: String) -> E;
    }

    impl<T: std::fmt::Debug, E> UnwrapErrOrElseMessage<T, E> for Result<T, E> {
        fn unwrap_err_or_else_message(self, msg: String) -> E {
            match self {
                Ok(v) => panic!("{msg}; got Ok({v:?})"),
                Err(e) => e,
            }
        }
    }
}
