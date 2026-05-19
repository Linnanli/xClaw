//! Boundary-level security tests for MCP server-name handling.
//!
//! Verifies that the `McpServerName` allowlist (restored in F3.2 phase 1)
//! is enforced through the *host* APIs that previously accepted raw
//! strings — specifically `McpServerConfig::validate`, which is the
//! single gatekeeper called by both the on-disk `mcp-servers.json` loader
//! (`load_mcp_servers_from`) and the `add_mcp_server` mutation API.
//!
//! See `gh issue view 628#issuecomment-4484408668` for the threat model
//! and ADR-152 §3 F3.2 for the broader port.

use ironclaw::tools::mcp::config::McpServerConfig;

fn cfg(name: &str) -> McpServerConfig {
    McpServerConfig::new(name, "https://example.com/mcp")
}

/// The allowlist must accept plain alphanumeric names plus the safe
/// separators (`-`, `_`, `.`) — this is the **happy path** that exercises
/// the parts of the API that downstream code (`secrets/<name>`,
/// `Mcp-Session-Id` header) implicitly depends on.
#[test]
fn req_security_mcp_server_name_accepts_normal_names() {
    for ok in [
        "github",
        "notion",
        "my-server",
        "my_server",
        "server.example",
        "Server1",
    ] {
        cfg(ok)
            .validate()
            .unwrap_or_else(|e| panic!("plain name '{ok}' must validate, got: {e}"));
    }
}

#[test]
fn test_security_server_name_rejects_path_traversal() {
    // These would let a malicious config escape the per-server secrets
    // directory (`~/.dasclaw/secrets/<name>/...`) or be misread as a
    // directory entry.
    for bad in ["..", ".", "../etc", "/etc/passwd", "a/b", "a\\b"] {
        let err = cfg(bad)
            .validate()
            .expect_err(&format!("path traversal '{bad}' must be rejected"));
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("invalid mcp server name"),
            "expected allowlist error for {bad:?}, got: {err}"
        );
    }
}

#[test]
fn test_security_server_name_rejects_control_chars() {
    // CR/LF would let a malicious config inject extra HTTP header lines
    // (header smuggling) when the name flows into `Mcp-Session-Id`.
    for bad in ["a\nb", "a\rb", "a\tb", "a\0b", "a\x07b"] {
        let err = cfg(bad)
            .validate()
            .expect_err(&format!("control char in {bad:?} must be rejected"));
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("invalid mcp server name"),
            "expected allowlist error for {bad:?}, got: {err}"
        );
    }
}

#[test]
fn test_security_server_name_rejects_non_ascii() {
    for bad in ["café", "服务器", "emoji-🎯"] {
        let err = cfg(bad)
            .validate()
            .expect_err(&format!("non-ascii {bad:?} must be rejected"));
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("invalid mcp server name"),
            "expected allowlist error for {bad:?}, got: {err}"
        );
    }
}

#[test]
fn test_security_server_name_rejects_shell_meta() {
    // Spaces / shell metacharacters could be problematic when the name
    // is interpolated into log lines or — historically — into stdio
    // command arguments.
    for bad in [
        "a b", "a;b", "a|b", "a&b", "a$b", "a`b", "a(b", "a)b", "a<b", "a>b",
    ] {
        let err = cfg(bad)
            .validate()
            .expect_err(&format!("shell meta in {bad:?} must be rejected"));
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("invalid mcp server name"),
            "expected allowlist error for {bad:?}, got: {err}"
        );
    }
}

#[test]
fn test_security_server_name_rejects_empty() {
    let err = cfg("").validate().expect_err("empty name must be rejected");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("invalid mcp server name"),
        "expected allowlist error, got: {err}"
    );
}

/// `legacy_token_secret_name()` provides the pre-normalization fallback
/// used by `auth::get_access_token` / `auth::is_authenticated`. The
/// contract is:
///
/// 1. A name with **no** underscores has no legacy form (`None`).
/// 2. A normalized name (underscores in place of hyphens) maps back to
///    the canonical hyphenated form, prefixed with `mcp_` and suffixed
///    with `_access_token` — matching the layout `ironclaw-main` uses.
#[test]
fn req_security_legacy_secret_name_fallback_reads_old_name() {
    // No-underscore canonical names have no legacy form.
    assert_eq!(cfg("github").legacy_token_secret_name(), None);
    assert_eq!(cfg("notion").legacy_token_secret_name(), None);

    // Normalized names map back to the hyphenated original.
    assert_eq!(
        cfg("my_server").legacy_token_secret_name().as_deref(),
        Some("mcp_my-server_access_token"),
    );
    assert_eq!(
        cfg("multi_part_name").legacy_token_secret_name().as_deref(),
        Some("mcp_multi-part-name_access_token"),
    );
}
