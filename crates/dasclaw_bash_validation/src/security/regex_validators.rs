//! Slice 2.1.c1 — 9 regex / state-machine main validators.
//!
//! 1:1 port of upstream `bashSecurity.ts` validators that operate purely on
//! string views (no AST decoding):
//!
//! - [`validate_shell_metacharacters`] (L783-L822, check_id 5)
//! - [`validate_dangerous_variables`] (L823-L845, check_id 6)
//! - [`validate_dangerous_patterns`] (L846-L874, check_ids 8 / backtick)
//! - [`validate_redirections`] (L875-L903, check_ids 9 / 10)
//! - [`validate_ifs_injection`] (L1017-L1039, check_id 11)
//! - [`validate_proc_environ_access`] (L1041-L1079, check_id 13)
//! - [`validate_mid_word_hash`] (L1919-L1988, check_id 19)
//! - [`validate_comment_quote_desync`] (L1990-L2107, check_id 22)
//! - [`validate_quoted_newline`] (L2109-L2184, check_id 23)
//!
//! All `behavior: 'ask'` upstream outcomes are collapsed to
//! [`SecurityResult::Block`] in Phase 2.1 per plan §0 (Porting model);
//! the deferred-non-misparsing engine still preserves upstream's surfaced
//! [`DecisionReason`] precedence (see `mod.rs`).

use once_cell::sync::Lazy;
use regex::Regex;

use super::context::ValidationContext;
use super::quote_extract::has_unescaped_char;
use super::types::{DecisionReason, SecurityCheckId, SecurityResult};

// =========================================================================
// Static patterns
// =========================================================================

/// Upstream `COMMAND_SUBSTITUTION_PATTERNS` (L14-L42). Each entry maps to a
/// human-readable substring of the upstream message format
/// `"Command contains <message>"`.
struct CommandSubstitutionPattern {
    re: Regex,
    message: &'static str,
}

static COMMAND_SUBSTITUTION_PATTERNS: Lazy<Vec<CommandSubstitutionPattern>> = Lazy::new(|| {
    let entries: &[(&str, &str)] = &[
        (r"<\(", "process substitution <()"),
        (r">\(", "process substitution >()"),
        (r"=\(", "Zsh process substitution =()"),
        // Zsh EQUALS expansion: =cmd at word start (not VAR=val).
        (r"(?:^|[\s;&|])=[a-zA-Z_]", "Zsh equals expansion (=cmd)"),
        (r"\$\(", "$() command substitution"),
        (r"\$\{", "${} parameter substitution"),
        (r"\$\[", "$[] legacy arithmetic expansion"),
        (r"~\[", "Zsh-style parameter expansion"),
        (r"\(e:", "Zsh-style glob qualifiers"),
        (r"\(\+", "Zsh glob qualifier with command execution"),
        (
            r"\}\s*always\s*\{",
            "Zsh always block (try/always construct)",
        ),
        (r"<#", "PowerShell comment syntax"),
    ];
    entries
        .iter()
        .map(|(p, m)| CommandSubstitutionPattern {
            re: Regex::new(p).expect("static regex"), // safety: literal pattern, compile-time validated
            message: m,
        })
        .collect()
});

// validateShellMetacharacters sub-patterns (L791, L798-L802, L815)
static SHELL_META_QUOTED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|\s)["'][^"']*[;&][^"']*["'](?:\s|$)"#).expect("static regex")
    // safety: literal pattern
});
static SHELL_META_NAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"-name\s+["'][^"']*[;|&][^"']*["']"#).expect("static regex") // safety: static literal pattern, compile-time validated
});
static SHELL_META_PATH: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"-path\s+["'][^"']*[;|&][^"']*["']"#).expect("static regex") // safety: static literal pattern, compile-time validated
});
static SHELL_META_INAME: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"-iname\s+["'][^"']*[;|&][^"']*["']"#).expect("static regex") // safety: static literal pattern, compile-time validated
});
static SHELL_META_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"-regex\s+["'][^"']*[;&][^"']*["']"#).expect("static regex") // safety: static literal pattern, compile-time validated
});

// validateDangerousVariables (L828-L831)
static DANGEROUS_VAR_REDIR_TO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[<>|]\s*\$[A-Za-z_]").expect("static regex") // safety: static literal pattern, compile-time validated
});
static DANGEROUS_VAR_REDIR_FROM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*\s*[|<>]").expect("static regex") // safety: static literal pattern, compile-time validated
});

// validateIFSInjection (L1023)
static IFS_INJECTION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$IFS|\$\{[^}]*IFS").expect("static regex")); // safety: static literal pattern, compile-time validated

// validateProcEnvironAccess (L1057)
static PROC_ENVIRON_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"/proc/.*/environ").expect("static regex")); // safety: static literal pattern, compile-time validated

// =========================================================================
// Validators
// =========================================================================

fn injection(check_id: SecurityCheckId, sub_id: u32, message: &str) -> SecurityResult {
    SecurityResult::Block {
        reason: DecisionReason::CommandInjection {
            check_id,
            sub_id,
            message: message.to_string(),
        },
    }
}

/// Upstream `validateShellMetacharacters` (L783-L822). 3 sub-IDs.
pub fn validate_shell_metacharacters(ctx: &ValidationContext) -> SecurityResult {
    let unquoted = ctx.unquoted_content();
    let msg = "Command contains shell metacharacters (;, |, or &) in arguments";

    if SHELL_META_QUOTED.is_match(unquoted) {
        return injection(SecurityCheckId::ShellMetacharacters, 1, msg);
    }
    if SHELL_META_NAME.is_match(unquoted)
        || SHELL_META_PATH.is_match(unquoted)
        || SHELL_META_INAME.is_match(unquoted)
    {
        return injection(SecurityCheckId::ShellMetacharacters, 2, msg);
    }
    if SHELL_META_REGEX.is_match(unquoted) {
        return injection(SecurityCheckId::ShellMetacharacters, 3, msg);
    }
    SecurityResult::Passthrough
}

/// Upstream `validateDangerousVariables` (L823-L845).
pub fn validate_dangerous_variables(ctx: &ValidationContext) -> SecurityResult {
    let fully_unquoted = ctx.fully_unquoted_content();
    if DANGEROUS_VAR_REDIR_TO.is_match(fully_unquoted)
        || DANGEROUS_VAR_REDIR_FROM.is_match(fully_unquoted)
    {
        return injection(
            SecurityCheckId::DangerousVariables,
            1,
            "Command contains variables in dangerous contexts (redirections or pipes)",
        );
    }
    SecurityResult::Passthrough
}

/// Upstream `validateDangerousPatterns` (L846-L874).
///
/// Reports check_id `DangerousPatternsCommandSubstitution` (8) for all
/// branches. Sub-IDs:
///   - `0` — unescaped backtick (upstream emits no `logEvent` here; we assign
///     `0` to distinguish from the regex-pattern branch)
///   - `1` — any [`COMMAND_SUBSTITUTION_PATTERNS`] hit
pub fn validate_dangerous_patterns(ctx: &ValidationContext) -> SecurityResult {
    let unquoted = ctx.unquoted_content();

    if has_unescaped_char(unquoted, '`') {
        return injection(
            SecurityCheckId::DangerousPatternsCommandSubstitution,
            0,
            "Command contains backticks (`) for command substitution",
        );
    }

    for entry in COMMAND_SUBSTITUTION_PATTERNS.iter() {
        if entry.re.is_match(unquoted) {
            return injection(
                SecurityCheckId::DangerousPatternsCommandSubstitution,
                1,
                &format!("Command contains {}", entry.message),
            );
        }
    }
    SecurityResult::Passthrough
}

/// Upstream `validateRedirections` (L875-L903). Two distinct check IDs.
pub fn validate_redirections(ctx: &ValidationContext) -> SecurityResult {
    let fully_unquoted = ctx.fully_unquoted_content();

    if fully_unquoted.contains('<') {
        return injection(
            SecurityCheckId::DangerousPatternsInputRedirection,
            1,
            "Command contains input redirection (<) which could read sensitive files",
        );
    }
    if fully_unquoted.contains('>') {
        return injection(
            SecurityCheckId::DangerousPatternsOutputRedirection,
            1,
            "Command contains output redirection (>) which could write to arbitrary files",
        );
    }
    SecurityResult::Passthrough
}

/// Upstream `validateIFSInjection` (L1017-L1039).
pub fn validate_ifs_injection(ctx: &ValidationContext) -> SecurityResult {
    if IFS_INJECTION_RE.is_match(ctx.original_command()) {
        return injection(
            SecurityCheckId::IfsInjection,
            1,
            "Command contains IFS variable usage which could bypass security validation",
        );
    }
    SecurityResult::Passthrough
}

/// Upstream `validateProcEnvironAccess` (L1041-L1079).
pub fn validate_proc_environ_access(ctx: &ValidationContext) -> SecurityResult {
    if PROC_ENVIRON_RE.is_match(ctx.original_command()) {
        return injection(
            SecurityCheckId::ProcEnvironAccess,
            1,
            "Command accesses /proc/*/environ which could expose sensitive environment variables",
        );
    }
    SecurityResult::Passthrough
}

/// Upstream `validateMidWordHash` (L1919-L1962).
///
/// Detects `#` preceded by a non-whitespace char (mid-word hash) — a
/// shell-quote vs bash parser differential. Excludes `${#` (bash string-length).
/// Also checks the continuation-joined form to catch backslash-newline bypasses.
pub fn validate_mid_word_hash(ctx: &ValidationContext) -> SecurityResult {
    let keep = ctx.unquoted_keep_quote_chars();
    if mid_word_hash_present(keep) || mid_word_hash_present(&join_continuations(keep)) {
        return injection(
            SecurityCheckId::MidWordHash,
            1,
            "Command contains mid-word # which is parsed differently by shell-quote vs bash",
        );
    }
    SecurityResult::Passthrough
}

/// Replicates upstream's `/\S(?<!\$\{)#/` lookbehind: returns true iff there
/// is a `#` whose preceding character is non-whitespace AND the two chars
/// before that `#` are not `${`.
fn mid_word_hash_present(s: &str) -> bool {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'#' || i == 0 {
            continue;
        }
        let prev = bytes[i - 1];
        if prev.is_ascii_whitespace() {
            continue;
        }
        // Exclude `${#` — i.e. prev=='{' and bytes[i-2]=='$'
        if prev == b'{' && i >= 2 && bytes[i - 2] == b'$' {
            continue;
        }
        return true;
    }
    false
}

/// Mirrors upstream's `/\\+\n/g` continuation-join (L1944-L1946):
/// odd backslash count → drop the trailing backslash and the newline;
/// even backslash count → leave the match unchanged.
fn join_continuations(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // Count consecutive backslashes
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let count = i - start;
            // Followed by '\n'?
            if i < bytes.len() && bytes[i] == b'\n' {
                if count % 2 == 1 {
                    // odd → drop one backslash + the newline
                    out.push_str(&"\\".repeat(count - 1));
                } else {
                    out.push_str(&"\\".repeat(count));
                    out.push('\n');
                }
                i += 1; // skip the newline
            } else {
                out.push_str(&"\\".repeat(count));
            }
            continue;
        }
        // SAFETY: bytes[i] is ASCII or start of a UTF-8 sequence; we copy by char.
        let ch_start = i;
        let ch_len = utf8_char_len(bytes[i]);
        let end = (ch_start + ch_len).min(bytes.len());
        // safety: indices ch_start..end are valid UTF-8 char boundaries by construction
        out.push_str(&s[ch_start..end]);
        i = end;
    }
    out
}

fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        b if b < 0x80 => 1,
        b if b < 0xC0 => 1, // invalid leading byte; advance by 1 to make progress
        b if b < 0xE0 => 2,
        b if b < 0xF0 => 3,
        _ => 4,
    }
}

/// Upstream `validateCommentQuoteDesync` (L1990-L2106).
///
/// Per upstream L1994-L2000: when tree-sitter is the authoritative quote
/// context (which is always the case in our port), this validator is a
/// pure passthrough — the regex-tracker desync that this check defends
/// against cannot occur when the AST is the source of truth.
pub fn validate_comment_quote_desync(_ctx: &ValidationContext) -> SecurityResult {
    // Tree-sitter is always available in our port (single AST source per
    // plan §4.2). The desync the upstream regex path defends against is
    // structurally impossible when the AST is authoritative.
    SecurityResult::Passthrough
}

/// Upstream `validateQuotedNewline` (L2109-L2184).
///
/// Detects an in-quotes newline followed by a line whose `trim()` starts
/// with `#` — exactly the trigger that upstream `stripCommentLines` would
/// drop, hiding arguments from downstream path validation.
pub fn validate_quoted_newline(ctx: &ValidationContext) -> SecurityResult {
    let cmd = ctx.original_command();

    // Fast path: must have both '\n' and '#' somewhere.
    if !cmd.contains('\n') || !cmd.contains('#') {
        return SecurityResult::Passthrough;
    }

    let bytes = cmd.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if b == b'\\' && !in_single {
            escaped = true;
            i += 1;
            continue;
        }

        if b == b'\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }

        if b == b'"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }

        if b == b'\n' && (in_single || in_double) {
            // Find the next line's bounds.
            let line_start = i + 1;
            let next_nl = cmd[line_start..]
                .find('\n')
                .map(|p| line_start + p)
                .unwrap_or(cmd.len());
            let next_line = &cmd[line_start..next_nl];
            if next_line.trim_start().starts_with('#') {
                return injection(
                    SecurityCheckId::QuotedNewline,
                    1,
                    "Command contains a quoted newline followed by a #-prefixed line, which can hide arguments from line-based permission checks",
                );
            }
        }
        i += 1;
    }

    SecurityResult::Passthrough
}
