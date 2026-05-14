//! Slice 2.1.a — five **early** validators.
//!
//! Order matches upstream `bashSecurity.ts` (L2257-L2424) so differential
//! testing can compare on a check-by-check basis.
//!
//! These run **before** the AST is consulted and reject obviously hostile
//! syntactic shapes (tabs, lone flags, control characters, U+00A0-style
//! invisible whitespace). They establish the Fail-Safe baseline that Phase
//! 2.1.b's main validators rely on.

use once_cell::sync::Lazy;
use regex::Regex;

use super::context::ValidationContext;
use super::types::{DecisionReason, SecurityCheckId, SecurityResult};

// ─────────────────────────── helpers ────────────────────────────────────────

fn block(check_id: SecurityCheckId, sub_id: u32, message: &str) -> SecurityResult {
    SecurityResult::Block {
        reason: DecisionReason::CommandInjection {
            check_id,
            sub_id,
            message: message.to_string(),
        },
    }
}

// ─────────────────────────── validate_empty ─────────────────────────────────

/// Upstream `validateEmpty` (L233-L242). An empty command is explicitly safe
/// and short-circuits the entire pipeline.
pub fn validate_empty(ctx: &ValidationContext) -> SecurityResult {
    if ctx.original_command().trim().is_empty() {
        return SecurityResult::Allow;
    }
    SecurityResult::Passthrough
}

// ─────────────────────────── validate_incomplete_commands ───────────────────

static INCOMPLETE_TAB_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*\t").expect("static regex") // safety: literal pattern, build-time correctness
});
static INCOMPLETE_OPERATOR_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*(&&|\|\||;|>>?|<)").expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateIncompleteCommands` (L244-L292). Three sub-checks:
/// 1. starts with TAB → likely a continuation of a previous line
/// 2. trimmed starts with `-` → looks like just flags
/// 3. starts with an operator → looks like a continuation
pub fn validate_incomplete_commands(ctx: &ValidationContext) -> SecurityResult {
    let original = ctx.original_command();

    if INCOMPLETE_TAB_START.is_match(original) {
        return block(
            SecurityCheckId::IncompleteCommands,
            1,
            "Command appears to be an incomplete fragment (starts with tab)",
        );
    }

    if original.trim_start().starts_with('-') {
        return block(
            SecurityCheckId::IncompleteCommands,
            2,
            "Command appears to be an incomplete fragment (starts with flags)",
        );
    }

    if INCOMPLETE_OPERATOR_START.is_match(original) {
        return block(
            SecurityCheckId::IncompleteCommands,
            3,
            "Command appears to be a continuation line (starts with operator)",
        );
    }

    SecurityResult::Passthrough
}

// ─────────────────────────── validate_newlines ──────────────────────────────

/// Upstream `validateNewlines` (L905-L943). Scans
/// `fully_unquoted_pre_strip` (so newlines inside `'…'` / `"…"` do not fire
/// — matching upstream) for an LF/CR followed by a non-whitespace byte,
/// permitting `<whitespace>\<newline>` (a bash line continuation at a word
/// boundary) as the **only** exception.
///
/// Rust's `regex` crate has no look-behind, so we replicate the upstream
/// `/(?<![\s]\\)[\n\r]\s*\S/` semantics with a manual byte walk.
pub fn validate_newlines(ctx: &ValidationContext) -> SecurityResult {
    let bytes = ctx.fully_unquoted_pre_strip().as_bytes();
    if !bytes.iter().any(|b| *b == b'\n' || *b == b'\r') {
        return SecurityResult::Passthrough;
    }

    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' || b == b'\r' {
            let preceded_by_safe_cont =
                i >= 2 && bytes[i - 1] == b'\\' && bytes[i - 2].is_ascii_whitespace();
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let followed_by_non_ws = j < bytes.len();
            if followed_by_non_ws && !preceded_by_safe_cont {
                return block(
                    SecurityCheckId::Newlines,
                    1,
                    "Command contains newlines that could separate multiple commands",
                );
            }
        }
        i += 1;
    }

    SecurityResult::Passthrough
}

// ─────────────────────────── validate_carriage_return ───────────────────────

/// Upstream `validateCarriageReturn` (L971-L1015). Carriage return outside of
/// **double** quotes is a misparsing concern (shell-quote vs bash IFS
/// differential) — block on first occurrence.
pub fn validate_carriage_return(ctx: &ValidationContext) -> SecurityResult {
    let original = ctx.original_command();
    if !original.contains('\r') {
        return SecurityResult::Passthrough;
    }

    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for c in original.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if !in_single => {
                escaped = true;
            }
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '\r' if !in_double => {
                return block(
                    SecurityCheckId::Newlines,
                    2,
                    "Command contains carriage return (\\r) which shell-quote and bash tokenize differently",
                );
            }
            _ => {}
        }
    }

    SecurityResult::Passthrough
}

// ─────────────────────────── validate_unicode_whitespace ────────────────────

/// Upstream `UNICODE_WS_RE` (L1899) — the precise set of unicode whitespace
/// codepoints that shell-quote treats as word separators but bash does not.
fn is_unicode_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// Upstream `validateUnicodeWhitespace` (L1902-L1917).
pub fn validate_unicode_whitespace(ctx: &ValidationContext) -> SecurityResult {
    if ctx.original_command().chars().any(is_unicode_whitespace) {
        return block(
            SecurityCheckId::UnicodeWhitespace,
            1,
            "Command contains Unicode whitespace characters that could cause parsing inconsistencies",
        );
    }
    SecurityResult::Passthrough
}

// ─────────────────────────── validate_git_commit ────────────────────────────

static GIT_COMMIT_PREFIX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^git\s+commit\s+").expect("static regex") // safety: literal pattern, build-time correctness
});

/// Double-quoted message: captures (1) content, (2) remainder.
static GIT_COMMIT_MSG_DQ_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)^git[ \t]+commit[ \t]+[^;&|`$<>()\n\r]*?-m[ \t]+"(.*?)"(.*)$"#)
        .expect("static regex") // safety: literal pattern, build-time correctness
});

/// Single-quoted message: captures (1) content, (2) remainder.
static GIT_COMMIT_MSG_SQ_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)^git[ \t]+commit[ \t]+[^;&|`$<>()\n\r]*?-m[ \t]+'(.*?)'(.*)$"#)
        .expect("static regex") // safety: literal pattern, build-time correctness
});

static GIT_COMMIT_MSG_SUBST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\(|`|\$\{").expect("static regex") // safety: literal pattern, build-time correctness
});

static GIT_COMMIT_REMAINDER_OPS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[;|&()`]|\$\(|\$\{").expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateGitCommit` (L612-L741). Early validator that returns
/// `Allow` for safe `git commit -m "..."` invocations, falling through to
/// the main pipeline otherwise. Sub-ids:
///   1 → GIT_COMMIT_SUBSTITUTION (double-quoted message contains `$(`, `` ` ``, `${`)
///   5 → OBFUSCATED_FLAGS (message starts with `-`)
pub fn validate_git_commit(ctx: &ValidationContext) -> SecurityResult {
    let original = ctx.original_command();

    if ctx.base_command() != "git" || !GIT_COMMIT_PREFIX_RE.is_match(original) {
        return SecurityResult::Passthrough;
    }

    // Backslash anywhere → bail to full validators (regex quote tracking
    // cannot survive `\"` / `\'`).
    if original.contains('\\') {
        return SecurityResult::Passthrough;
    }

    let Some((quote, message, remainder)) = (|| {
        if let Some(caps) = GIT_COMMIT_MSG_DQ_RE.captures(original) {
            return Some((
                "\"",
                caps.get(1).map(|m| m.as_str()).unwrap_or(""),
                caps.get(2).map(|m| m.as_str()).unwrap_or(""),
            ));
        }
        if let Some(caps) = GIT_COMMIT_MSG_SQ_RE.captures(original) {
            return Some((
                "'",
                caps.get(1).map(|m| m.as_str()).unwrap_or(""),
                caps.get(2).map(|m| m.as_str()).unwrap_or(""),
            ));
        }
        None
    })() else {
        return SecurityResult::Passthrough;
    };

    // Double-quoted message containing command substitution patterns.
    if quote == "\"" && !message.is_empty() && GIT_COMMIT_MSG_SUBST_RE.is_match(message) {
        return block(
            SecurityCheckId::GitCommitSubstitution,
            1,
            "Git commit message contains command substitution patterns",
        );
    }

    // Remainder contains shell metacharacters that could chain commands.
    if !remainder.is_empty() && GIT_COMMIT_REMAINDER_OPS_RE.is_match(remainder) {
        return SecurityResult::Passthrough;
    }
    if !remainder.is_empty() {
        // Strip quoted content from remainder, then check for unquoted `<`/`>`.
        // Reachable only when the original command contains NO backslash (the
        // backslash bail above), so simple quote toggling is correct.
        let mut unquoted = String::new();
        let mut in_sq = false;
        let mut in_dq = false;
        for c in remainder.chars() {
            match c {
                '\'' if !in_dq => in_sq = !in_sq,
                '"' if !in_sq => in_dq = !in_dq,
                _ if !in_sq && !in_dq => unquoted.push(c),
                _ => {}
            }
        }
        if unquoted.contains('<') || unquoted.contains('>') {
            return SecurityResult::Passthrough;
        }
    }

    // Message starts with `-` → looks like an obfuscated flag.
    if message.starts_with('-') {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            5,
            "Command contains quoted characters in flag names",
        );
    }

    // Safe git commit form — short-circuit the rest of the pipeline.
    SecurityResult::Allow
}
