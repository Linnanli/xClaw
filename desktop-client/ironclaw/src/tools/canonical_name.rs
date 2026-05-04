//! Canonical tool-name namespace strategy (issue #87).
//!
//! Defines a deterministic identity for every tool that flows through
//! [`crate::tools::registry::ToolRegistry`]:
//!
//! | Source bucket  | Canonical form              | Example                    |
//! |----------------|-----------------------------|----------------------------|
//! | Built-in       | bare name                   | `shell`, `read_file`       |
//! | MCP server     | `mcp.<server>.<tool>`       | `mcp.github.create_issue`  |
//! | WASM extension | `wasm.<package>.<tool>`     | `wasm.acme.lint`           |
//! | Extension      | `ext.<slug>.<tool>`         | `ext.acme-suite.formatter` |
//!
//! Display labels (user-friendly UI strings) live separately on the [`Tool`]
//! impl; the canonical name is what the registry, policy, audit log, and
//! prompt metadata must agree on.
//!
//! # Why this lives next to the registry
//!
//! Issue #88 already records `(name, source, outcome)` per registration. This
//! module contributes the "what does the name *mean*" half so the registry
//! can:
//!
//! 1. Reject **dynamic-vs-dynamic** collisions (two MCP servers fighting over
//!    `mcp.foo.bar`, two WASM packages fighting over `wasm.foo.bar`, …).
//! 2. Classify each registration into a finer source bucket
//!    (`Mcp`/`Wasm`/`Extension`) for the startup report instead of the
//!    catch-all `Dynamic`.
//!
//! # Non-goals (deferred)
//!
//! - **Strict prefix enforcement**: existing dynamic registrations that lack
//!   a namespace prefix continue to work (parsed as
//!   [`CanonicalKind::LegacyDynamic`]). Tightening to "every dynamic tool
//!   MUST be namespaced" would break in-flight WASM tools and belongs in a
//!   follow-up once all call-sites are migrated.
//! - **Display name plumbing**: `Tool::display_name()` etc. is a separate
//!   refactor. The current registration report carries canonical names only;
//!   display names will be added when every `Tool` impl is updated.

use std::fmt;

/// Coarse classification used by the registry / report to bucket a
/// registration without depending on the heavier
/// [`crate::tools::registration_report::ToolSource`].
///
/// Kept `#[non_exhaustive]` so additional buckets (e.g. `Routine`, `Skill`)
/// can land without breaking external matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CanonicalKind {
    /// Bare-name built-in tool (registered via the sync startup path).
    Builtin,
    /// `mcp.<server>.<tool>`.
    Mcp,
    /// `wasm.<package>.<tool>`.
    Wasm,
    /// `ext.<slug>.<tool>`.
    Extension,
    /// Dynamic tool that did not adopt any reserved namespace prefix.
    /// Parsed permissively for backwards compatibility.
    LegacyDynamic,
}

impl CanonicalKind {
    /// Stable string label suitable for log fields and JSON keys.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Mcp => "mcp",
            Self::Wasm => "wasm",
            Self::Extension => "extension",
            Self::LegacyDynamic => "legacy_dynamic",
        }
    }
}

/// Reserved namespace prefixes. Centralised here so the parser, the registry,
/// and any audit/UI helper agree.
pub const NAMESPACE_PREFIX_MCP: &str = "mcp";
pub const NAMESPACE_PREFIX_WASM: &str = "wasm";
pub const NAMESPACE_PREFIX_EXTENSION: &str = "ext";

/// Parsed canonical tool name.
///
/// Stores the original raw string verbatim plus parsed-out segments so callers
/// don't need to re-split. Equality and hashing are based on the raw form,
/// which is the invariant the registry HashMap uses as its key.
#[derive(Debug, Clone)]
pub struct CanonicalToolName {
    raw: String,
    kind: CanonicalKind,
    /// First namespace segment (server / package / slug). `None` for
    /// `Builtin` and `LegacyDynamic`.
    authority: Option<String>,
    /// Final tool segment. Always set; equals `raw` for built-ins and
    /// legacy-dynamic tools.
    leaf: String,
}

impl CanonicalToolName {
    /// Parse a name string with an explicit hint about whether it came from
    /// the built-in startup path.
    ///
    /// Built-in tools always classify as [`CanonicalKind::Builtin`] regardless
    /// of their bare name; this avoids false-positive `mcp.…` matches if a
    /// future built-in ever uses a dotted name.
    pub fn parse(name: &str, is_builtin: bool) -> Self {
        let raw = name.to_string();
        if is_builtin {
            return Self {
                leaf: raw.clone(),
                raw,
                kind: CanonicalKind::Builtin,
                authority: None,
            };
        }

        // Split into at most 3 segments: prefix . authority . leaf
        // (the leaf itself may contain further dots, kept verbatim).
        let mut parts = name.splitn(3, '.');
        let prefix = parts.next().unwrap_or("");
        let authority = parts.next();
        let leaf = parts.next();

        match (prefix, authority, leaf) {
            (NAMESPACE_PREFIX_MCP, Some(auth), Some(tool))
                if !auth.is_empty() && !tool.is_empty() =>
            {
                Self {
                    raw,
                    kind: CanonicalKind::Mcp,
                    authority: Some(auth.to_string()),
                    leaf: tool.to_string(),
                }
            }
            (NAMESPACE_PREFIX_WASM, Some(auth), Some(tool))
                if !auth.is_empty() && !tool.is_empty() =>
            {
                Self {
                    raw,
                    kind: CanonicalKind::Wasm,
                    authority: Some(auth.to_string()),
                    leaf: tool.to_string(),
                }
            }
            (NAMESPACE_PREFIX_EXTENSION, Some(auth), Some(tool))
                if !auth.is_empty() && !tool.is_empty() =>
            {
                Self {
                    raw,
                    kind: CanonicalKind::Extension,
                    authority: Some(auth.to_string()),
                    leaf: tool.to_string(),
                }
            }
            _ => Self {
                leaf: raw.clone(),
                raw,
                kind: CanonicalKind::LegacyDynamic,
                authority: None,
            },
        }
    }

    /// Raw canonical string — the value stored as the registry HashMap key.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn kind(&self) -> CanonicalKind {
        self.kind
    }

    /// First namespace segment (server name for MCP, package for WASM,
    /// extension slug for `ext.*`). `None` for built-ins and legacy-dynamic.
    pub fn authority(&self) -> Option<&str> {
        self.authority.as_deref()
    }

    /// Tool-leaf segment (the final dotted segment, or the whole name for
    /// built-ins / legacy-dynamic).
    pub fn leaf(&self) -> &str {
        &self.leaf
    }
}

impl fmt::Display for CanonicalToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl PartialEq for CanonicalToolName {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for CanonicalToolName {}

impl std::hash::Hash for CanonicalToolName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_87_parse_builtin_bare_name() {
        let c = CanonicalToolName::parse("shell", true);
        assert_eq!(c.kind(), CanonicalKind::Builtin);
        assert_eq!(c.as_str(), "shell");
        assert_eq!(c.leaf(), "shell");
        assert!(c.authority().is_none());
    }

    #[test]
    fn req_87_parse_mcp_three_segments() {
        let c = CanonicalToolName::parse("mcp.github.create_issue", false);
        assert_eq!(c.kind(), CanonicalKind::Mcp);
        assert_eq!(c.authority(), Some("github"));
        assert_eq!(c.leaf(), "create_issue");
        assert_eq!(c.as_str(), "mcp.github.create_issue");
    }

    #[test]
    fn req_87_parse_wasm_three_segments() {
        let c = CanonicalToolName::parse("wasm.acme.lint", false);
        assert_eq!(c.kind(), CanonicalKind::Wasm);
        assert_eq!(c.authority(), Some("acme"));
        assert_eq!(c.leaf(), "lint");
    }

    #[test]
    fn req_87_parse_extension_three_segments() {
        let c = CanonicalToolName::parse("ext.acme-suite.formatter", false);
        assert_eq!(c.kind(), CanonicalKind::Extension);
        assert_eq!(c.authority(), Some("acme-suite"));
        assert_eq!(c.leaf(), "formatter");
    }

    #[test]
    fn req_87_parse_dotted_leaf_kept_verbatim() {
        // splitn(3) means the leaf can itself contain dots, e.g. an MCP
        // server that namespaces tools internally.
        let c = CanonicalToolName::parse("mcp.github.repo.create", false);
        assert_eq!(c.kind(), CanonicalKind::Mcp);
        assert_eq!(c.authority(), Some("github"));
        assert_eq!(c.leaf(), "repo.create");
    }

    #[test]
    fn req_87_parse_legacy_when_prefix_missing() {
        let c = CanonicalToolName::parse("my_legacy_tool", false);
        assert_eq!(c.kind(), CanonicalKind::LegacyDynamic);
        assert_eq!(c.leaf(), "my_legacy_tool");
        assert!(c.authority().is_none());
    }

    #[test]
    fn req_87_parse_legacy_when_prefix_segments_missing() {
        // "mcp.foo" lacks a tool segment → not a valid canonical MCP name,
        // falls back to LegacyDynamic.
        let c = CanonicalToolName::parse("mcp.foo", false);
        assert_eq!(c.kind(), CanonicalKind::LegacyDynamic);
    }

    #[test]
    fn req_87_parse_legacy_when_authority_empty() {
        // "mcp..tool" must not silently classify as MCP with empty server.
        let c = CanonicalToolName::parse("mcp..tool", false);
        assert_eq!(c.kind(), CanonicalKind::LegacyDynamic);
    }

    #[test]
    fn req_87_parse_legacy_when_unknown_prefix() {
        let c = CanonicalToolName::parse("foo.bar.baz", false);
        assert_eq!(c.kind(), CanonicalKind::LegacyDynamic);
    }

    #[test]
    fn req_87_builtin_hint_overrides_dotted_form() {
        // A built-in registered with a dotted name still classifies as
        // Builtin because the startup path called `register_sync`.
        let c = CanonicalToolName::parse("mcp.shadow", true);
        assert_eq!(c.kind(), CanonicalKind::Builtin);
    }

    #[test]
    fn req_87_kind_as_str_stable() {
        assert_eq!(CanonicalKind::Builtin.as_str(), "builtin");
        assert_eq!(CanonicalKind::Mcp.as_str(), "mcp");
        assert_eq!(CanonicalKind::Wasm.as_str(), "wasm");
        assert_eq!(CanonicalKind::Extension.as_str(), "extension");
        assert_eq!(CanonicalKind::LegacyDynamic.as_str(), "legacy_dynamic");
    }

    #[test]
    fn req_87_equality_and_hash_use_raw_form() {
        use std::collections::HashSet;
        let a = CanonicalToolName::parse("mcp.gh.list", false);
        let b = CanonicalToolName::parse("mcp.gh.list", false);
        assert_eq!(a, b);
        let mut set: HashSet<CanonicalToolName> = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
