//! Quote / redirection extraction helpers — Slice 2.1.b port of upstream
//! `extractQuotedContent`, `stripSafeRedirections`, `hasUnescapedChar`
//! (`bashSecurity.ts` L122-L228).
//!
//! These produce the three lossy views of the command that the main
//! validators (Slice 2.1.c) consume:
//!
//! | view                          | content inside `'…'` | content inside `"…"` | quote delimiters |
//! |-------------------------------|----------------------|----------------------|------------------|
//! | `with_double_quotes`          | dropped              | kept                 | dropped          |
//! | `fully_unquoted`              | dropped              | dropped              | dropped          |
//! | `unquoted_keep_quote_chars`   | dropped              | dropped              | **kept**         |
//!
//! Slice 2.1.b keeps these as plain `String` so Slice 2.1.c can index into
//! them with regex / byte-walks identically to the upstream JS.

use once_cell::sync::Lazy;
use regex::Regex;

/// Output of [`extract_quoted_content`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteExtraction {
    pub with_double_quotes: String,
    pub fully_unquoted: String,
    pub unquoted_keep_quote_chars: String,
}

/// Verbatim port of upstream `extractQuotedContent` (`bashSecurity.ts`
/// L130-L175). Single-pass state machine — see the module-level table for the
/// semantics of each of the three returned views.
///
/// The `is_jq` flag matches upstream: when the base command is `jq`, the
/// double-quote delimiter is **also** copied into `with_double_quotes` /
/// `fully_unquoted` so that jq filter content stays available to the jq
/// validator (`validateJqCommand`, Slice 2.1.c).
pub fn extract_quoted_content(command: &str, is_jq: bool) -> QuoteExtraction {
    let mut with_double_quotes = String::with_capacity(command.len());
    let mut fully_unquoted = String::with_capacity(command.len());
    let mut unquoted_keep_quote_chars = String::with_capacity(command.len());
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            escaped = false;
            if !in_single_quote {
                with_double_quotes.push(ch);
            }
            if !in_single_quote && !in_double_quote {
                fully_unquoted.push(ch);
                unquoted_keep_quote_chars.push(ch);
            }
            continue;
        }

        if ch == '\\' && !in_single_quote {
            escaped = true;
            // Upstream pushes `\` unconditionally when !in_single_quote.
            with_double_quotes.push(ch);
            if !in_double_quote {
                fully_unquoted.push(ch);
                unquoted_keep_quote_chars.push(ch);
            }
            continue;
        }

        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            unquoted_keep_quote_chars.push(ch);
            continue;
        }

        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            unquoted_keep_quote_chars.push(ch);
            if !is_jq {
                continue;
            }
            // For jq: fall through so the `"` is also recorded into the
            // other two views below.
        }

        if !in_single_quote {
            with_double_quotes.push(ch);
        }
        if !in_single_quote && !in_double_quote {
            fully_unquoted.push(ch);
            unquoted_keep_quote_chars.push(ch);
        }
    }

    QuoteExtraction {
        with_double_quotes,
        fully_unquoted,
        unquoted_keep_quote_chars,
    }
}

// ─────────────────────────── strip_safe_redirections ────────────────────────

// SECURITY (mirrors upstream comment at L178-L188): every pattern below MUST
// end with `(?:\s|$)` so a prefix match such as `> /dev/nullo` does NOT strip
// `> /dev/null`. Without the trailing boundary `validateRedirections` would
// observe `echo hi o` and pass while the original command silently writes to
// `/dev/nullo`.
static REDIR_STDERR_TO_STDOUT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\s+2\s*>&\s*1(?:\s|$)").expect("static regex") // safety: literal pattern, build-time correctness
});
static REDIR_TO_DEV_NULL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[012]?\s*>\s*/dev/null(?:\s|$)").expect("static regex") // safety: literal pattern, build-time correctness
});
static REDIR_FROM_DEV_NULL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\s*<\s*/dev/null(?:\s|$)").expect("static regex") // safety: literal pattern, build-time correctness
});

/// Verbatim port of upstream `stripSafeRedirections` (L196-L209).
pub fn strip_safe_redirections(content: &str) -> String {
    let s1 = REDIR_STDERR_TO_STDOUT.replace_all(content, "");
    let s2 = REDIR_TO_DEV_NULL.replace_all(&s1, "");
    REDIR_FROM_DEV_NULL.replace_all(&s2, "").into_owned()
}

// ─────────────────────────── has_unescaped_char ─────────────────────────────

/// Verbatim port of upstream `hasUnescapedChar` (L220-L248). Walks `content`
/// looking for an occurrence of `ch` that is not preceded by an odd number
/// of backslashes.
///
/// Caller is responsible for passing a single bash-meaningful character
/// (e.g. `` ` ``, `$`, `;`). Multi-character search is intentionally not
/// supported — see upstream security note about ANSI-C quoting bypasses.
pub fn has_unescaped_char(content: &str, ch: char) -> bool {
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // Skip the next char (the escape target).
            if chars.next().is_none() {
                return false;
            }
            continue;
        }
        if c == ch {
            return true;
        }
    }
    false
}
