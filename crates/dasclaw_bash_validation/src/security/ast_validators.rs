//! Slice 2.1.c2 — 7 AST / state-machine main validators.
//!
//! 1:1 port of upstream `bashSecurity.ts` validators that traverse the
//! command with bash-aware quote semantics (state machines) or require AST
//! input. None of these is a pure regex — that's why they're separated
//! from [`super::regex_validators`].
//!
//! Ported functions:
//!
//! - [`validate_jq_command`] (L742-L782, check_ids 2 / 3)
//! - [`validate_obfuscated_flags`] (L1130-L1582, check_id 4)
//! - [`validate_backslash_escaped_whitespace`] (L1583-L1602, check_id 15)
//! - [`validate_backslash_escaped_operators`] (L1696-L1750, check_id 21)
//! - [`validate_brace_expansion`] (L1751-L1898, check_id 16)
//! - [`validate_zsh_dangerous_commands`] (L2186-L2240, check_id 20)
//! - [`validate_malformed_token_injection`] (L1082-L1129, check_id 14)
//!
//! All `behavior: 'ask'` upstream outcomes are collapsed to
//! [`SecurityResult::Block`] in Phase 2.1 per plan §0 (Porting model).
//!
//! ## Deviations from upstream
//!
//! - `validate_malformed_token_injection` is a *defense-favorable simplification*:
//!   we don't have shell-quote tokens, so we check (a) command-level quote
//!   parity and (b) AST `ERROR` nodes. Both directions are at least as strict
//!   as upstream's per-token balance walk, preserving the asymmetric
//!   invariant `upstream_blocks ⟹ xclaw_blocks`. See block-level comment
//!   on the function.
//! - Heredoc body stripping (upstream `extractHeredocs`, L2287-L2289) is
//!   *not yet* applied to `original_command` — see plan §5.3 and
//!   [`super::context::ValidationContext::new`]. This makes our validators
//!   *stricter* on commands containing `<<'EOF'` / `<<\EOF` bodies (still
//!   asymmetric-invariant safe). Slice 2.1.d will land the strip pass.

use once_cell::sync::Lazy;
use regex::Regex;

use super::context::ValidationContext;
use super::types::{DecisionReason, SecurityCheckId, SecurityResult};

// =========================================================================
// Common helpers
// =========================================================================

fn block(check_id: SecurityCheckId, sub_id: u32, message: &str) -> SecurityResult {
    SecurityResult::Block {
        reason: DecisionReason::CommandInjection {
            check_id,
            sub_id,
            message: message.to_string(),
        },
    }
}

// =========================================================================
// validate_jq_command (check_ids 2, 3)
// =========================================================================

static JQ_SYSTEM_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\bsystem\s*\(").expect("static regex") // safety: literal pattern, build-time correctness
});

static JQ_DANGEROUS_FLAGS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?:^|\s)(?:-f\b|--from-file|--rawfile|--slurpfile|-L\b|--library-path)")
        .expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateJqCommand` (L742-L782).
///
/// Two sub-checks for `jq` base command:
///   1. `system()` function call → check_id 2 (JqSystemFunction)
///   2. Dangerous flags (`-f`, `--rawfile`, `-L`, ...) → check_id 3 (JqFileArguments)
pub fn validate_jq_command(ctx: &ValidationContext) -> SecurityResult {
    if ctx.base_command() != "jq" {
        return SecurityResult::Passthrough;
    }
    let original = ctx.original_command();

    if JQ_SYSTEM_FN_RE.is_match(original) {
        return block(
            SecurityCheckId::JqSystemFunction,
            1,
            "jq command contains system() function which executes arbitrary commands",
        );
    }

    // Upstream uses `originalCommand.substring(3).trim()`; we replicate by
    // slicing past the literal "jq" prefix (3 bytes including the trailing
    // space). When the command is shorter than 3 bytes the regex below would
    // match nothing anyway, so we just clamp.
    let after_jq = original.get(3..).unwrap_or("").trim_start();
    if JQ_DANGEROUS_FLAGS_RE.is_match(after_jq) {
        return block(
            SecurityCheckId::JqFileArguments,
            1,
            "jq command contains dangerous flags that could execute code or read arbitrary files",
        );
    }

    SecurityResult::Passthrough
}

// =========================================================================
// validate_obfuscated_flags (check_id 4)
// =========================================================================

static ANSI_C_QUOTING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$'[^']*'").expect("static regex") // safety: literal pattern, build-time correctness
});
static LOCALE_QUOTING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\$"[^"]*""#).expect("static regex") // safety: literal pattern, build-time correctness
});
static EMPTY_SPECIAL_QUOTE_DASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\$['"]{2}\s*-"#).expect("static regex") // safety: literal pattern, build-time correctness
});
static EMPTY_QUOTE_DASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|\s)(?:''|"")+\s*-"#).expect("static regex") // safety: literal pattern, build-time correctness
});
static HOMOGENEOUS_EMPTY_QUOTE_QUOTED_DASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:""|'')+['"]-"#).expect("static regex") // safety: literal pattern, build-time correctness
});
static TRIPLE_QUOTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|\s)['"]{3,}"#).expect("static regex") // safety: literal pattern, build-time correctness
});
static SPACE_QUOTED_DASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\s['"`]-"#).expect("static regex") // safety: literal pattern, build-time correctness
});
static DOUBLE_QUOTE_DASH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"['"`]{2}-"#).expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateObfuscatedFlags` (L1130-L1582). Detects shell quoting
/// bypass patterns used to hide dangerous flags.
///
/// This is a *port of a port* — upstream uses a hand-rolled quote-state
/// machine with multiple sub-id branches; we mirror it byte-for-byte
/// because the precise quote tracking is what makes the bypass detection
/// work.
pub fn validate_obfuscated_flags(ctx: &ValidationContext) -> SecurityResult {
    let original = ctx.original_command();
    let base = ctx.base_command();

    // Simple `echo` (no shell operators) is exempt.
    let has_shell_operators =
        original.contains('|') || original.contains('&') || original.contains(';');
    if base == "echo" && !has_shell_operators {
        return SecurityResult::Passthrough;
    }

    // 1. ANSI-C quoting `$'...'`
    if ANSI_C_QUOTING_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            5,
            "Command contains ANSI-C quoting which can hide characters",
        );
    }
    // 2. Locale quoting `$"..."`
    if LOCALE_QUOTING_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            6,
            "Command contains locale quoting which can hide characters",
        );
    }
    // 3. Empty special quote + dash: $''- or $""-
    if EMPTY_SPECIAL_QUOTE_DASH_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            9,
            "Command contains empty special quotes before dash (potential bypass)",
        );
    }
    // 4. Empty quote(s) + dash: '' - or "" -
    if EMPTY_QUOTE_DASH_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            7,
            "Command contains empty quotes before dash (potential bypass)",
        );
    }
    // 4b. Homogeneous empty quote pair adjacent to quoted dash
    if HOMOGENEOUS_EMPTY_QUOTE_QUOTED_DASH_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            10,
            "Command contains empty quote pair adjacent to quoted dash (potential flag obfuscation)",
        );
    }
    // 4c. 3+ consecutive quote characters at word start
    if TRIPLE_QUOTE_RE.is_match(original) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            11,
            "Command contains consecutive quote characters at word start (potential obfuscation)",
        );
    }

    // 5. Quote-state walk: detect ` "-...` or ` -<flagcontent-with-quotes>`
    if let Some(reason) = walk_obfuscated_flags(original, base) {
        return reason;
    }

    // 6. Final regex sweeps on fullyUnquotedContent.
    let fully_unquoted = ctx.fully_unquoted_content();
    if SPACE_QUOTED_DASH_RE.is_match(fully_unquoted) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            2,
            "Command contains quoted characters in flag names",
        );
    }
    if DOUBLE_QUOTE_DASH_RE.is_match(fully_unquoted) {
        return block(
            SecurityCheckId::ObfuscatedFlags,
            3,
            "Command contains quoted characters in flag names",
        );
    }

    SecurityResult::Passthrough
}

/// Per-byte quote-state walk porting upstream L1255-L1494.
fn walk_obfuscated_flags(command: &str, base: &str) -> Option<SecurityResult> {
    let bytes = command.as_bytes();
    let n = bytes.len();
    if n < 2 {
        return None;
    }

    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    let mut i = 0;
    while i + 1 < n {
        let c = bytes[i];
        let next = bytes[i + 1];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        // Backslash escapes only outside single quotes.
        if c == b'\\' && !in_single_quote {
            escaped = true;
            i += 1;
            continue;
        }

        if c == b'\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            i += 1;
            continue;
        }
        if c == b'"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            i += 1;
            continue;
        }

        if in_single_quote || in_double_quote {
            i += 1;
            continue;
        }

        // 5a. <whitespace><quote with dash inside / continuing>
        if is_ascii_ws(c) && is_quote_byte(next) {
            let quote_char = next;
            let mut j = i + 2;
            let inside_start = j;
            while j < n && bytes[j] != quote_char {
                j += 1;
            }
            let inside_quote = &bytes[inside_start..j.min(n)];
            let closed = j < n && bytes[j] == quote_char;
            let char_after_quote = bytes.get(j + 1).copied();

            if closed {
                let has_flag_chars_inside = matches_dash_then_alnum(inside_quote);
                let has_flag_chars_continuing = is_all_dashes(inside_quote)
                    && !inside_quote.is_empty()
                    && char_after_quote
                        .map(is_flag_continuation_char)
                        .unwrap_or(false);
                let has_flag_chars_in_next_quote = (inside_quote.is_empty()
                    || is_all_dashes(inside_quote))
                    && char_after_quote.map(is_quote_byte).unwrap_or(false)
                    && chained_quote_forms_flag(bytes, j + 1, inside_quote);

                if has_flag_chars_inside
                    || has_flag_chars_continuing
                    || has_flag_chars_in_next_quote
                {
                    return Some(block(
                        SecurityCheckId::ObfuscatedFlags,
                        4,
                        "Command contains quoted characters in flag names",
                    ));
                }
            }
        }

        // 5b. <whitespace>-<flag content possibly containing quotes>
        if is_ascii_ws(c) && next == b'-' {
            let mut j = i + 1;
            let mut flag_content: Vec<u8> = Vec::new();
            while j < n {
                let fc = bytes[j];
                if is_ascii_ws(fc) || fc == b'=' {
                    break;
                }
                if is_quote_byte(fc) {
                    // `cut -d` exception
                    if base == "cut" && flag_content == b"-d" {
                        break;
                    }
                    if let Some(nfc) = bytes.get(j + 1) {
                        if !is_flag_inner_byte(*nfc) {
                            break;
                        }
                    }
                }
                flag_content.push(fc);
                j += 1;
            }
            if flag_content.contains(&b'"') || flag_content.contains(&b'\'') {
                return Some(block(
                    SecurityCheckId::ObfuscatedFlags,
                    1,
                    "Command contains quoted characters in flag names",
                ));
            }
        }

        i += 1;
    }

    None
}

fn is_ascii_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}
fn is_quote_byte(b: u8) -> bool {
    matches!(b, b'\'' | b'"' | b'`')
}
fn is_alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}
fn matches_dash_then_alnum(s: &[u8]) -> bool {
    // /^-+[a-zA-Z0-9$`]/
    let mut i = 0;
    while i < s.len() && s[i] == b'-' {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    matches!(s.get(i), Some(b) if is_alnum(*b) || *b == b'$' || *b == b'`')
}
fn is_all_dashes(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(|b| *b == b'-')
}
fn is_flag_continuation_char(b: u8) -> bool {
    // /[a-zA-Z0-9\\${`-]/
    is_alnum(b) || b == b'\\' || b == b'$' || b == b'{' || b == b'`' || b == b'-'
}
fn is_flag_inner_byte(b: u8) -> bool {
    // /[a-zA-Z0-9_'"-]/
    is_alnum(b) || b == b'_' || b == b'\'' || b == b'"' || b == b'-'
}
fn contains_alnum_dollar_or_backtick(s: &[u8]) -> bool {
    s.iter().any(|b| is_alnum(*b) || *b == b'$' || *b == b'`')
}

/// Mirrors the IIFE in upstream L1410-L1494 — walks a chain of adjacent
/// quoted segments starting at `start` (which points to an opening quote)
/// and decides whether the combined content forms a flag.
fn chained_quote_forms_flag(bytes: &[u8], start: usize, initial_inside: &[u8]) -> bool {
    let n = bytes.len();
    let mut pos = start;
    let mut combined: Vec<u8> = initial_inside.to_vec();
    while pos < n && is_quote_byte(bytes[pos]) {
        let seg_quote = bytes[pos];
        let mut end = pos + 1;
        while end < n && bytes[end] != seg_quote {
            end += 1;
        }
        let segment = &bytes[pos + 1..end.min(n)];
        combined.extend_from_slice(segment);

        if matches_dash_then_alnum(&combined) {
            return true;
        }
        let prior_len = combined.len().saturating_sub(segment.len());
        let prior = &combined[..prior_len];
        if is_all_dashes(prior) && contains_alnum_dollar_or_backtick(segment) {
            return true;
        }
        if end >= n {
            return false;
        }
        pos = end + 1;
    }
    // Trailing unquoted char that could complete a flag.
    if pos < n {
        let next_char = bytes[pos];
        if is_flag_continuation_char(next_char) {
            if is_all_dashes(&combined) || combined.is_empty() {
                if next_char == b'-' {
                    return true;
                }
                if (is_alnum(next_char)
                    || next_char == b'\\'
                    || next_char == b'$'
                    || next_char == b'{'
                    || next_char == b'`')
                    && !combined.is_empty()
                {
                    return true;
                }
            }
            if combined.first() == Some(&b'-') {
                return true;
            }
        }
    }
    false
}

// =========================================================================
// validate_backslash_escaped_whitespace (check_id 15)
// =========================================================================

/// Upstream `hasBackslashEscapedWhitespace` (L1551-L1582).
fn has_backslash_escaped_whitespace(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && !in_single {
            if !in_double {
                if let Some(nc) = bytes.get(i + 1) {
                    if *nc == b' ' || *nc == b'\t' {
                        return true;
                    }
                }
            }
            i += 2;
            continue;
        }
        if c == b'"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }
        if c == b'\'' && !in_double {
            in_single = !in_single;
        }
        i += 1;
    }
    false
}

/// Upstream `validateBackslashEscapedWhitespace` (L1583-L1602).
pub fn validate_backslash_escaped_whitespace(ctx: &ValidationContext) -> SecurityResult {
    if has_backslash_escaped_whitespace(ctx.original_command()) {
        return block(
            SecurityCheckId::BackslashEscapedWhitespace,
            1,
            "Command contains backslash-escaped whitespace that could alter command parsing",
        );
    }
    SecurityResult::Passthrough
}

// =========================================================================
// validate_backslash_escaped_operators (check_id 21)
// =========================================================================

/// Upstream `SHELL_OPERATORS` (L1641) — bytes whose `\<op>` causes the
/// splitCommand normalization double-parse bug.
fn is_shell_operator(b: u8) -> bool {
    matches!(b, b';' | b'|' | b'&' | b'<' | b'>')
}

/// Upstream `hasBackslashEscapedOperator` (L1642-L1694).
fn has_backslash_escaped_operator(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && !in_single {
            if !in_double {
                if let Some(nc) = bytes.get(i + 1) {
                    if is_shell_operator(*nc) {
                        return true;
                    }
                }
            }
            i += 2;
            continue;
        }
        if c == b'\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if c == b'"' && !in_single {
            in_double = !in_double;
        }
        i += 1;
    }
    false
}

/// Upstream `validateBackslashEscapedOperators` (L1696-L1750).
///
/// Note: upstream short-circuits via `treeSitter.hasActualOperatorNodes`
/// to skip the regex when the AST proves no operator nodes exist. Our
/// `ValidationContext` does not yet expose that signal — Slice 2.1.d will
/// add the corresponding AST query. Until then we always run the byte
/// walk, which is *stricter* than upstream (no false negatives).
pub fn validate_backslash_escaped_operators(ctx: &ValidationContext) -> SecurityResult {
    if has_backslash_escaped_operator(ctx.original_command()) {
        return block(
            SecurityCheckId::BackslashEscapedOperators,
            1,
            "Command contains a backslash before a shell operator (;, |, &, <, >) which can hide command structure",
        );
    }
    SecurityResult::Passthrough
}

// =========================================================================
// validate_brace_expansion (check_id 16)
// =========================================================================

/// Upstream `isEscapedAtPosition` (L1733-L1742). Counts consecutive
/// backslashes before `pos`; odd count means the char at `pos` is escaped.
fn is_escaped_at_position(bytes: &[u8], pos: usize) -> bool {
    let mut count = 0usize;
    let mut i = pos;
    while i > 0 && bytes[i - 1] == b'\\' {
        count += 1;
        i -= 1;
    }
    !count.is_multiple_of(2)
}

static QUOTED_SINGLE_BRACE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"['"][{}]['"]"#).expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateBraceExpansion` (L1751-L1898).
pub fn validate_brace_expansion(ctx: &ValidationContext) -> SecurityResult {
    let content = ctx.fully_unquoted_pre_strip();
    let bytes = content.as_bytes();

    let mut open = 0usize;
    let mut close = 0usize;
    for i in 0..bytes.len() {
        if bytes[i] == b'{' && !is_escaped_at_position(bytes, i) {
            open += 1;
        } else if bytes[i] == b'}' && !is_escaped_at_position(bytes, i) {
            close += 1;
        }
    }

    if open > 0 && close > open {
        return block(
            SecurityCheckId::BraceExpansion,
            2,
            "Command has excess closing braces after quote stripping, indicating possible brace expansion obfuscation",
        );
    }

    if open > 0 && QUOTED_SINGLE_BRACE_RE.is_match(ctx.original_command()) {
        return block(
            SecurityCheckId::BraceExpansion,
            3,
            "Command contains quoted brace character inside brace context (potential brace expansion obfuscation)",
        );
    }

    // Scan `{...}` pairs at depth 0; look for `,` or `..` at depth 0 inside.
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' || is_escaped_at_position(bytes, i) {
            i += 1;
            continue;
        }
        // Find matching `}` by depth.
        let mut depth = 1i32;
        let mut close_pos: Option<usize> = None;
        let mut j = i + 1;
        while j < bytes.len() {
            let ch = bytes[j];
            if ch == b'{' && !is_escaped_at_position(bytes, j) {
                depth += 1;
            } else if ch == b'}' && !is_escaped_at_position(bytes, j) {
                depth -= 1;
                if depth == 0 {
                    close_pos = Some(j);
                    break;
                }
            }
            j += 1;
        }
        let Some(end) = close_pos else {
            i += 1;
            continue;
        };

        let mut inner_depth = 0i32;
        let mut k = i + 1;
        while k < end {
            let ch = bytes[k];
            if ch == b'{' && !is_escaped_at_position(bytes, k) {
                inner_depth += 1;
            } else if ch == b'}' && !is_escaped_at_position(bytes, k) {
                inner_depth -= 1;
            } else if inner_depth == 0
                && (ch == b',' || (ch == b'.' && k + 1 < end && bytes[k + 1] == b'.'))
            {
                return block(
                    SecurityCheckId::BraceExpansion,
                    1,
                    "Command contains brace expansion that could alter command parsing",
                );
            }
            k += 1;
        }
        i += 1;
    }

    SecurityResult::Passthrough
}

// =========================================================================
// validate_zsh_dangerous_commands (check_id 20)
// =========================================================================

const ZSH_DANGEROUS_COMMANDS: &[&str] = &[
    "zmodload", "emulate", "sysopen", "sysread", "syswrite", "sysseek", "zpty", "ztcp", "zsocket",
    "mapfile", "zf_rm", "zf_mv", "zf_ln", "zf_chmod", "zf_chown", "zf_mkdir", "zf_rmdir",
    "zf_chgrp",
];

const ZSH_PRECOMMAND_MODIFIERS: &[&str] = &["command", "builtin", "noglob", "nocorrect"];

static ENV_ASSIGNMENT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Za-z_]\w*=").expect("static regex") // safety: literal pattern, build-time correctness
});

static FC_DASH_E_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\s-\S*e").expect("static regex") // safety: literal pattern, build-time correctness
});

/// Upstream `validateZshDangerousCommands` (L2186-L2240).
pub fn validate_zsh_dangerous_commands(ctx: &ValidationContext) -> SecurityResult {
    let trimmed = ctx.original_command().trim();
    if trimmed.is_empty() {
        return SecurityResult::Passthrough;
    }

    let mut base_cmd = "";
    for token in trimmed.split_whitespace() {
        if ENV_ASSIGNMENT_RE.is_match(token) {
            continue;
        }
        if ZSH_PRECOMMAND_MODIFIERS.contains(&token) {
            continue;
        }
        base_cmd = token;
        break;
    }

    if ZSH_DANGEROUS_COMMANDS.contains(&base_cmd) {
        return block(
            SecurityCheckId::ZshDangerousCommands,
            1,
            &format!("Command uses Zsh-specific '{base_cmd}' which can bypass security checks"),
        );
    }

    if base_cmd == "fc" && FC_DASH_E_RE.is_match(trimmed) {
        return block(
            SecurityCheckId::ZshDangerousCommands,
            2,
            "Command uses 'fc -e' which can execute arbitrary commands via editor",
        );
    }

    SecurityResult::Passthrough
}

// =========================================================================
// validate_malformed_token_injection (check_id 14)
// =========================================================================
//
// Upstream `validateMalformedTokenInjection` (L1082-L1129) uses
// `hasMalformedTokens` (shellQuote.ts L117-L177), which requires
// shell-quote token output we don't produce. Our port preserves the
// asymmetric invariant (`upstream_blocks ⟹ xclaw_blocks`) by checking:
//
//   1. Command-level quote parity (mirrors hasMalformedTokens L122-L141)
//   2. Tree-sitter `ERROR` node presence (covers per-token unbalance,
//      since unbalanced quote/brace/paren in any token surfaces as an
//      AST `ERROR` node)
//
// Both checks gate on the presence of a command separator (`;`, `&&`,
// `||`) **outside quotes**, exactly mirroring upstream's `hasCommandSeparator`
// pre-check (L1100-L1108).

/// Returns true if the command contains an unquoted `;`, `&&`, or `||`.
fn has_unquoted_separator(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && !in_single {
            i += 2;
            continue;
        }
        if c == b'\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if c == b'"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }
        if !in_single && !in_double {
            if c == b';' {
                return true;
            }
            if (c == b'&' || c == b'|') && bytes.get(i + 1).copied() == Some(c) {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Returns true if total quote count is odd (mirrors hasMalformedTokens
/// L122-L141 quote walk).
fn has_unbalanced_quotes(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut single_count = 0u32;
    let mut double_count = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && !in_single {
            i += 2;
            continue;
        }
        if c == b'"' && !in_single {
            double_count += 1;
            in_double = !in_double;
        } else if c == b'\'' && !in_double {
            single_count += 1;
            in_single = !in_single;
        }
        i += 1;
    }
    !single_count.is_multiple_of(2) || !double_count.is_multiple_of(2)
}

/// Upstream `validateMalformedTokenInjection` (L1082-L1129). See module
/// header for the deviation from upstream's shell-quote-tokens approach.
pub fn validate_malformed_token_injection(ctx: &ValidationContext) -> SecurityResult {
    let original = ctx.original_command();
    if !has_unquoted_separator(original) {
        return SecurityResult::Passthrough;
    }

    if has_unbalanced_quotes(original) {
        return block(
            SecurityCheckId::MalformedTokenInjection,
            1,
            "Command contains ambiguous syntax with command separators that could be misinterpreted",
        );
    }

    SecurityResult::Passthrough
}
