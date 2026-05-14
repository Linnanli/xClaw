//! Shell permission rule parsing + wildcard matching.
//!
//! Verbatim semantic port of `claude-code-main/src/utils/permissions/
//! shellRuleMatching.ts` (~230 LOC). Algorithm matches upstream
//! byte-for-byte; only language idioms differ (Rust `regex` crate vs JS
//! `RegExp`).
//!
//! ## Three rule shapes (discriminated by [`ShellPermissionRule`])
//!
//! 1. **Exact** — literal string, no wildcards (`git status`)
//! 2. **Prefix** — legacy `name:*` syntax (`npm:*` → prefix `npm`)
//! 3. **Wildcard** — contains unescaped `*` (`git diff *`)
//!
//! ## Escaping in wildcard patterns
//!
//! - `\*` matches a literal asterisk
//! - `\\` matches a literal backslash
//! - all other regex metacharacters are escaped automatically
//!
//! ## Trailing-wildcard optionalization (`git *` semantics)
//!
//! When a pattern ends in ` *` (space + unescaped wildcard) and that is
//! the **only** unescaped wildcard, the trailing space-and-args is made
//! optional so `git *` matches both `git add` and bare `git`. Aligns
//! wildcard rules with the legacy `git:*` prefix shape. Multi-wildcard
//! patterns (e.g. `* run *`) are excluded so `npm run *` does not
//! incorrectly match bare `npm run` with no trailing argument.

use regex::Regex;

/// Sentinel chars used to defer escaped-wildcard handling past the regex
/// metachar escaping pass. PUA (Private Use Area) so they cannot
/// collide with real pattern content.
const ESC_STAR: char = '\u{E000}';
const ESC_BACKSLASH: char = '\u{E001}';

/// Parsed permission-rule shape.
///
/// Mirrors upstream `ShellPermissionRule` discriminated union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPermissionRule {
    /// Literal command string, matched by equality.
    Exact { command: String },
    /// Legacy `name:*` prefix syntax — matches any command starting
    /// with `prefix` followed by whitespace (or empty tail).
    Prefix { prefix: String },
    /// Glob pattern containing `*` wildcards.
    Wildcard { pattern: String },
}

/// Options controlling wildcard matching behavior.
///
/// Bash itself is case-sensitive; `case_insensitive` is preserved for
/// forward-compat with upstream's `caseInsensitive` parameter (used by
/// non-Bash callers that share this module).
#[derive(Debug, Clone, Copy, Default)]
pub struct MatchOptions {
    pub case_insensitive: bool,
}

/// Extract the prefix from legacy `name:*` syntax.
///
/// Returns `Some("npm")` for `"npm:*"`, `None` otherwise.
#[must_use]
pub fn permission_rule_extract_prefix(rule: &str) -> Option<&str> {
    rule.strip_suffix(":*").filter(|s| !s.is_empty())
}

/// Check if `pattern` contains an unescaped `*` that is not part of
/// trailing `:*` legacy syntax.
///
/// Returns `false` for `"npm:*"` (legacy prefix), `true` for `"npm *"`
/// or `"git diff *"`.
#[must_use]
pub fn has_wildcards(pattern: &str) -> bool {
    if pattern.ends_with(":*") {
        return false;
    }
    let bytes = pattern.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'*' {
            // Count preceding backslashes.
            let mut backslash_count = 0usize;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslash_count += 1;
                j -= 1;
            }
            if backslash_count.is_multiple_of(2) {
                return true;
            }
        }
    }
    false
}

/// Match a `command` against a wildcard `pattern`.
///
/// See module docs for escaping rules and trailing-wildcard semantics.
///
/// Returns `false` on the (theoretically impossible) event of an
/// internally-malformed regex — see `it_never_produces_an_invalid_regex`
/// test for the invariant.
#[must_use]
pub fn match_wildcard_pattern(pattern: &str, command: &str, opts: MatchOptions) -> bool {
    let trimmed = pattern.trim();

    // Pass 1: collapse `\*` and `\\` into PUA sentinels so the regex
    // metachar escape pass below cannot touch them.
    let mut processed = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some(&'*') => {
                    processed.push(ESC_STAR);
                    chars.next();
                    continue;
                }
                Some(&'\\') => {
                    processed.push(ESC_BACKSLASH);
                    chars.next();
                    continue;
                }
                _ => {}
            }
        }
        processed.push(c);
    }

    let unescaped_star_count = processed.chars().filter(|&c| c == '*').count();

    // Pass 2: build the final regex pattern, escaping metachars and
    // converting unescaped `*` → `.*`, PUA sentinels → literal escapes.
    let mut regex_pattern = String::with_capacity(processed.len() * 2);
    for c in processed.chars() {
        match c {
            '*' => regex_pattern.push_str(".*"),
            ESC_STAR => regex_pattern.push_str("\\*"),
            ESC_BACKSLASH => regex_pattern.push_str("\\\\"),
            '.' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\' | '\''
            | '"' => {
                regex_pattern.push('\\');
                regex_pattern.push(c);
            }
            _ => regex_pattern.push(c),
        }
    }

    // Trailing-wildcard optionalization: `git *` → `git( .*)?`.
    if regex_pattern.ends_with(" .*") && unescaped_star_count == 1 {
        let new_len = regex_pattern.len() - 3;
        regex_pattern.truncate(new_len);
        regex_pattern.push_str("( .*)?");
    }

    let flag_prefix = if opts.case_insensitive {
        "(?si)"
    } else {
        "(?s)"
    };
    let full = format!("{flag_prefix}^{regex_pattern}$");
    match Regex::new(&full) {
        Ok(re) => re.is_match(command),
        Err(_) => false,
    }
}

/// Parse a permission rule string into a [`ShellPermissionRule`].
///
/// Resolution order matches upstream:
/// 1. Legacy `:*` prefix wins over wildcard parsing (so `foo:*` is a
///    prefix, not a wildcard match against `foo:` followed by any tail)
/// 2. Otherwise, if pattern contains unescaped `*`, it's a wildcard
/// 3. Else it's an exact match
#[must_use]
pub fn parse_permission_rule(rule: &str) -> ShellPermissionRule {
    if let Some(prefix) = permission_rule_extract_prefix(rule) {
        return ShellPermissionRule::Prefix {
            prefix: prefix.to_string(),
        };
    }
    if has_wildcards(rule) {
        return ShellPermissionRule::Wildcard {
            pattern: rule.to_string(),
        };
    }
    ShellPermissionRule::Exact {
        command: rule.to_string(),
    }
}
