//! Permission rule parser — Slice 2.2.j (Issue #490 Phase 2.2).
//!
//! Semantic port of upstream
//! `claude-code-main/src/utils/permissions/permissionRuleParser.ts`
//! L1-L198 (TypeScript). Provides parsing/serialization of the
//! `"Tool"` / `"Tool(content)"` permission rule string format with
//! correct handling of escaped parentheses, plus a legacy-tool-name
//! alias map.
//!
//! ## Why a separate parser slice
//!
//! Storage shapes diverge between codebases:
//!   - upstream stores raw `"Bash(content)"` strings in settings JSON,
//!     parsed at every lookup;
//!   - our [`crate::types::PermissionRuleValue`] stores the parsed
//!     `(tool_name, rule_content)` pair eagerly.
//!
//! This module provides the **string ↔ struct** bridge so the
//! analyzer in Slice 2.2.i can be reached from any storage layer
//! (Slice 2.2.k will wire it into `permissions_loader`).
//!
//! ## ANT-only segment intentionally OMITTED
//!
//! Upstream L7-L17 + L26-L28 wire a `Brief` legacy alias behind the
//! `KAIROS` / `KAIROS_BRIEF` feature flag. This is the ANT-only
//! `BriefTool` and is forbidden by Issue #490 red line. The remaining
//! 4 aliases (`Task` → `Agent`, `KillShell` → `TaskStop`,
//! `AgentOutputTool` → `TaskOutput`, `BashOutputTool` → `TaskOutput`)
//! are upstream-canonical and ported verbatim.

/// Maps legacy tool names to their current canonical names.
///
/// Verbatim from upstream L20-L29, minus the ANT-only `Brief` entry.
/// **Order**: `(legacy, canonical)`.
const LEGACY_TOOL_NAME_ALIASES: &[(&str, &str)] = &[
    ("Task", "Agent"),                 // AGENT_TOOL_NAME
    ("KillShell", "TaskStop"),         // TASK_STOP_TOOL_NAME
    ("AgentOutputTool", "TaskOutput"), // TASK_OUTPUT_TOOL_NAME
    ("BashOutputTool", "TaskOutput"),  // TASK_OUTPUT_TOOL_NAME
];

/// Returns the canonical tool name for a possibly-legacy `name`.
///
/// Upstream `normalizeLegacyToolName` L31-L33. If `name` is not a
/// legacy alias the input is returned unchanged.
pub fn normalize_legacy_tool_name(name: &str) -> &str {
    for &(legacy, canonical) in LEGACY_TOOL_NAME_ALIASES {
        if legacy == name {
            return canonical;
        }
    }
    name
}

/// Returns all legacy aliases that resolve to `canonical_name`.
///
/// Upstream `getLegacyToolNames` L35-L41. The returned slice ordering
/// matches the [`LEGACY_TOOL_NAME_ALIASES`] declaration order, so
/// callers using this for UI display get a stable list.
pub fn get_legacy_tool_names(canonical_name: &str) -> Vec<&'static str> {
    let mut result = Vec::new();
    for &(legacy, canonical) in LEGACY_TOOL_NAME_ALIASES {
        if canonical == canonical_name {
            result.push(legacy);
        }
    }
    result
}

/// Escape `\`, `(`, `)` in `content` for safe storage inside a
/// `"Tool(content)"` permission-rule string.
///
/// Order matters (upstream L52-L59):
///   1. backslashes first (`\` → `\\`)
///   2. then parens (`(` → `\(`, `)` → `\)`)
///
/// `escape_rule_content` is the inverse of [`unescape_rule_content`].
pub fn escape_rule_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for ch in content.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '(' => out.push_str(r"\("),
            ')' => out.push_str(r"\)"),
            other => out.push(other),
        }
    }
    out
}

/// Unescape a `content` string previously produced by
/// [`escape_rule_content`].
///
/// Order matters (upstream L73-L80, **reverse** of escape):
///   1. parens first (`\(` → `(`, `\)` → `)`)
///   2. then backslashes (`\\` → `\`)
///
/// We hand-roll the loop instead of three sequential string-replace
/// passes (upstream's approach) because chained replaces in Rust
/// would force two extra allocations. The single-pass walk preserves
/// the same semantics — including the **doubled-backslash followed by
/// paren** case (`\\(` ⇒ `\(`, not `\` then literal `(`): we consume
/// `\\` as a unit first, then the bare `(` is a literal.
pub fn unescape_rule_content(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    while i < bytes.len() {
        // Try a two-byte escape sequence.
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'(' => {
                    out.push('(');
                    i += 2;
                    continue;
                }
                b')' => {
                    out.push(')');
                    i += 2;
                    continue;
                }
                b'\\' => {
                    out.push('\\');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Fall through: copy the next UTF-8 code-point verbatim.
        let ch = content[i..]
            .chars()
            .next()
            .expect("non-empty remaining slice has at least one char"); // safety: i < bytes.len() guarantees remaining slice has ≥1 char
        let n = ch.len_utf8();
        out.push(ch);
        i += n;
    }
    out
}

/// Parse a permission rule string into a `(tool_name, rule_content)`
/// pair.
///
/// Semantic port of upstream `permissionRuleValueFromString` L94-L137.
///
/// Format: `"ToolName"` or `"ToolName(content)"`. Content may
/// contain escaped parens (`\(`, `\)`).
///
/// **Fail-Safe**: when the string is malformed (no closing paren,
/// content after closing paren, empty tool name), the entire input is
/// treated as a tool name — never crashes, never partially-parses.
/// This matches upstream's "treat as tool name" branches verbatim.
///
/// **Special cases** (upstream L132-L135): empty content (`Bash()`)
/// and standalone wildcard content (`Bash(*)`) collapse to a
/// tool-wide rule with `rule_content = None`.
///
/// Returns `(tool_name, rule_content)` as owned strings; the caller
/// assembles them into a [`crate::types::PermissionRuleValue`] (kept
/// out of this module's API to preserve its independence from the
/// `types` module).
pub fn permission_rule_value_from_string(rule_string: &str) -> (String, Option<String>) {
    // Find first unescaped '('.
    let open_paren_index = match find_first_unescaped_char(rule_string, '(') {
        Some(i) => i,
        None => return (normalize_legacy_tool_name(rule_string).to_string(), None),
    };

    // Find last unescaped ')'.
    let close_paren_index = match find_last_unescaped_char(rule_string, ')') {
        Some(i) if i > open_paren_index => i,
        // Includes both `None` and `Some(i) where i <= open`.
        _ => return (normalize_legacy_tool_name(rule_string).to_string(), None),
    };

    // Closing paren must be at the very end (upstream L117-L120).
    if close_paren_index != rule_string.len() - 1 {
        return (normalize_legacy_tool_name(rule_string).to_string(), None);
    }

    let tool_name = &rule_string[..open_paren_index];
    let raw_content = &rule_string[open_paren_index + 1..close_paren_index];

    // Empty tool name (e.g. `(foo)`) — treat whole string as tool name.
    if tool_name.is_empty() {
        return (normalize_legacy_tool_name(rule_string).to_string(), None);
    }

    // Empty content or standalone `*` ⇒ tool-wide rule.
    if raw_content.is_empty() || raw_content == "*" {
        return (normalize_legacy_tool_name(tool_name).to_string(), None);
    }

    let rule_content = unescape_rule_content(raw_content);
    (
        normalize_legacy_tool_name(tool_name).to_string(),
        Some(rule_content),
    )
}

/// Serialize a `(tool_name, rule_content)` pair back to a permission
/// rule string.
///
/// Semantic port of upstream `permissionRuleValueToString` L147-L155.
///
/// `None` or empty content ⇒ bare tool name (e.g. `"Bash"`).
/// Non-empty content is escaped via [`escape_rule_content`].
pub fn permission_rule_value_to_string(tool_name: &str, rule_content: Option<&str>) -> String {
    match rule_content {
        None | Some("") => tool_name.to_string(),
        Some(c) => format!("{}({})", tool_name, escape_rule_content(c)),
    }
}

/// Find the byte index of the first unescaped occurrence of `target`.
///
/// A character is escaped iff preceded by an **odd** number of
/// backslashes. Upstream `findFirstUnescapedChar` L161-L178.
///
/// `target` must be ASCII (`(`, `)` in our usage); we work on bytes
/// for efficiency.
fn find_first_unescaped_char(s: &str, target: char) -> Option<usize> {
    debug_assert!(target.is_ascii(), "target must be ASCII");
    let target_b = target as u8;
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == target_b {
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslash_count += 1;
                j -= 1;
            }
            if backslash_count.is_multiple_of(2) {
                return Some(i);
            }
        }
    }
    None
}

/// Find the byte index of the last unescaped occurrence of `target`.
///
/// Upstream `findLastUnescapedChar` L184-L198.
fn find_last_unescaped_char(s: &str, target: char) -> Option<usize> {
    debug_assert!(target.is_ascii(), "target must be ASCII");
    let target_b = target as u8;
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).rev() {
        if bytes[i] == target_b {
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslash_count += 1;
                j -= 1;
            }
            if backslash_count.is_multiple_of(2) {
                return Some(i);
            }
        }
    }
    None
}
