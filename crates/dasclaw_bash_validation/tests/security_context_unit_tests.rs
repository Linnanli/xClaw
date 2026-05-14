//! Slice 2.1.b — unit tests for `quote_extract` helpers and the extended
//! [`ValidationContext`].
//!
//! Naming follows AGENTS.md `req_security_490_p2_1_b_*`. Tests are split
//! between:
//! - `quote_extract` happy/edge cases (single quote, double quote, escapes,
//!   jq mode, redirection stripping, unescaped-char walker)
//! - context construction (derived views match upstream semantics; AST cache
//!   succeeds / fails consistently)
//!
//! These tests pin the **upstream semantics** of the derived string views.
//! Slice 2.1.c validators depend on them being byte-identical to
//! `bashSecurity.ts`.

use dasclaw_bash_validation::security::{
    extract_quoted_content, has_unescaped_char, strip_safe_redirections, ParseFail,
    QuoteExtraction, ValidationContext,
};

// ─────────────────────────── extract_quoted_content ─────────────────────────

#[test]
fn req_security_490_p2_1_b_quote_extract_no_quotes_identity() {
    let q = extract_quoted_content("echo hello world", false);
    assert_eq!(q.with_double_quotes, "echo hello world");
    assert_eq!(q.fully_unquoted, "echo hello world");
    assert_eq!(q.unquoted_keep_quote_chars, "echo hello world");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_single_quote_strips_content() {
    let q = extract_quoted_content("echo 'rm -rf /'", false);
    // with_double_quotes drops content inside single quotes
    assert_eq!(q.with_double_quotes, "echo ");
    assert_eq!(q.fully_unquoted, "echo ");
    // unquoted_keep_quote_chars keeps the `'` delimiters
    assert_eq!(q.unquoted_keep_quote_chars, "echo ''");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_double_quote_kept_in_with_dq_only() {
    let q = extract_quoted_content("echo \"hello world\"", false);
    assert_eq!(q.with_double_quotes, "echo hello world");
    assert_eq!(q.fully_unquoted, "echo ");
    assert_eq!(q.unquoted_keep_quote_chars, "echo \"\"");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_nested_dq_in_sq_is_literal() {
    // Inside single quotes, `"` is a literal character — should NOT toggle
    // double-quote state. Bashismetic correctness check.
    let q = extract_quoted_content("echo 'a\"b' c", false);
    assert_eq!(q.with_double_quotes, "echo  c");
    assert_eq!(q.fully_unquoted, "echo  c");
    assert_eq!(q.unquoted_keep_quote_chars, "echo '' c");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_backslash_outside_quotes() {
    // `\` outside quotes escapes the next char; both chars are kept in
    // all three views.
    let q = extract_quoted_content(r"echo \$HOME", false);
    assert_eq!(q.with_double_quotes, r"echo \$HOME");
    assert_eq!(q.fully_unquoted, r"echo \$HOME");
    assert_eq!(q.unquoted_keep_quote_chars, r"echo \$HOME");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_backslash_inside_single_quote_is_literal() {
    // Inside `'…'`, `\` is a literal backslash — NOT an escape.
    let q = extract_quoted_content(r"echo 'a\nb'", false);
    assert_eq!(q.with_double_quotes, "echo ");
    assert_eq!(q.fully_unquoted, "echo ");
    assert_eq!(q.unquoted_keep_quote_chars, "echo ''");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_jq_mode_keeps_double_quotes() {
    // When base_command == jq, `"` is also recorded into with_double_quotes
    // and fully_unquoted (upstream L153-L161 behaviour).
    let q = extract_quoted_content(r#"jq ".foo""#, true);
    assert_eq!(q.with_double_quotes, "jq \".foo\"");
    // Only the closing `"` falls into fully_unquoted (when in_double_quote
    // has just been toggled back off); the opening `"` is skipped because
    // in_double_quote was true at the bottom block.
    assert_eq!(q.fully_unquoted, "jq \"");
    // unquoted_keep_quote_chars: opening `"` from the if-block + closing `"`
    // from both the if-block (toggled) AND the fall-through bottom block.
    assert_eq!(q.unquoted_keep_quote_chars, "jq \"\"\"");
}

#[test]
fn req_security_490_p2_1_b_quote_extract_empty_string() {
    let q = extract_quoted_content("", false);
    assert_eq!(
        q,
        QuoteExtraction {
            with_double_quotes: String::new(),
            fully_unquoted: String::new(),
            unquoted_keep_quote_chars: String::new(),
        }
    );
}

// ─────────────────────────── strip_safe_redirections ────────────────────────

#[test]
fn req_security_490_p2_1_b_strip_redir_2_to_1() {
    // `\s+` consumes the leading space; trailing boundary `\s|$` is `$`.
    assert_eq!(strip_safe_redirections("echo hi 2>&1"), "echo hi");
    // Trailing boundary is `\s` (the space after `2>&1`), which the regex
    // consumes — "next" survives.
    assert_eq!(strip_safe_redirections("echo hi 2>&1 next"), "echo hinext");
}

#[test]
fn req_security_490_p2_1_b_strip_redir_dev_null_stdout() {
    // `> /dev/null`: `[012]?` matches empty so the regex can start at the
    // leading space (consumed by `\s*`) — result has NO trailing space.
    assert_eq!(strip_safe_redirections("echo hi > /dev/null"), "echo hi");
    // `2> /dev/null`: `[012]?` anchors the match at the `2`; the leading
    // space is NOT consumed.
    assert_eq!(strip_safe_redirections("echo hi 2> /dev/null"), "echo hi ");
}

#[test]
fn req_security_490_p2_1_b_strip_redir_dev_null_stdin() {
    // `\s*<\s*/dev/null` — leading `\s*` consumes the space, so result has
    // no trailing space.
    assert_eq!(strip_safe_redirections("cat < /dev/null"), "cat");
}

#[test]
fn req_security_490_p2_1_b_strip_redir_boundary_prevents_prefix_match() {
    // SECURITY: `> /dev/nullo` MUST NOT be stripped (the prefix `/dev/null`
    // is followed by `o`, not a word boundary). See upstream L178-L188.
    assert_eq!(
        strip_safe_redirections("echo hi > /dev/nullo"),
        "echo hi > /dev/nullo"
    );
}

#[test]
fn req_security_490_p2_1_b_strip_redir_no_redirection_identity() {
    assert_eq!(strip_safe_redirections("echo hi"), "echo hi");
}

// ─────────────────────────── has_unescaped_char ─────────────────────────────

#[test]
fn req_security_490_p2_1_b_unescaped_char_finds_unescaped_backtick() {
    assert!(has_unescaped_char("echo `date`", '`'));
}

#[test]
fn req_security_490_p2_1_b_unescaped_char_skips_escaped_backtick() {
    assert!(!has_unescaped_char(r"echo \`date\`", '`'));
}

#[test]
fn req_security_490_p2_1_b_unescaped_char_double_backslash_then_target() {
    // `\\` is an escaped backslash; following `` ` `` is therefore unescaped.
    assert!(has_unescaped_char(r"echo \\`date`", '`'));
}

#[test]
fn req_security_490_p2_1_b_unescaped_char_not_present() {
    assert!(!has_unescaped_char("echo hello", '`'));
}

// ─────────────────────────── ValidationContext derived views ────────────────

#[test]
fn req_security_490_p2_1_b_context_views_match_upstream_for_quoted_command() {
    // SECURITY: this test pins the upstream split (`bashSecurity.ts`
    // L2299-L2306) — `unquoted_content` is `withDoubleQuotes` (NOT redirection-
    // stripped); only `fully_unquoted_content` is stripped.
    let ctx = ValidationContext::new("echo 'secret' \"public\" > /dev/null");
    assert_eq!(
        ctx.original_command(),
        "echo 'secret' \"public\" > /dev/null"
    );
    assert_eq!(ctx.base_command(), "echo");
    // unquoted_content keeps the redirection (matches upstream).
    assert_eq!(ctx.unquoted_content(), "echo  public > /dev/null");
    // fully_unquoted_pre_strip: single+double stripped, redirection kept.
    assert_eq!(ctx.fully_unquoted_pre_strip(), "echo   > /dev/null");
    // fully_unquoted_content: the leading run of spaces lets the regex
    // match starting at position 4 (`\s*` consumes "   "), leaving "echo".
    assert_eq!(ctx.fully_unquoted_content(), "echo");
    // delimiters retained (`''` for the stripped single quote, `""` for the
    // stripped double quote).
    assert_eq!(ctx.unquoted_keep_quote_chars(), "echo '' \"\" > /dev/null");
}

#[test]
fn req_security_490_p2_1_b_context_base_command_uses_space_split() {
    // Upstream uses `split(' ')[0]` (space-only), NOT `split_whitespace()`.
    // A leading tab therefore leaves base_command empty (and
    // validate_incomplete_commands sub 1 will Block it).
    let ctx = ValidationContext::new("\tls -la");
    assert_eq!(ctx.base_command(), "\tls");
    let ctx = ValidationContext::new("");
    assert_eq!(ctx.base_command(), "");
}

#[test]
fn req_security_490_p2_1_b_context_ast_cached_ok() {
    let ctx = ValidationContext::new("echo hi");
    let ast = ctx.ast().expect("simple command parses");
    assert!(!ast.root_has_error());
}

#[test]
fn req_security_490_p2_1_b_context_ast_cached_err_is_stable() {
    let ctx = ValidationContext::new("echo 'unterminated");
    let err1 = ctx.ast().unwrap_err();
    let err2 = ctx.ast().unwrap_err();
    assert_eq!(err1, &ParseFail::ErrorNode);
    // Same error reference on both calls (cached, not re-parsed).
    assert!(std::ptr::eq(err1, err2));
}

#[test]
fn req_security_490_p2_1_b_context_jq_command_keeps_double_quotes_in_unquoted() {
    // Upstream L153-L161: when isJq, the `"` toggle still records into
    // `unquotedKeepQuoteChars` AND falls through to the bottom block. Inside
    // a double-quoted region the bottom `!inSingleQuote && !inDoubleQuote`
    // gate is FALSE, so the opening `"` is NOT echoed into fully_unquoted;
    // only the closing `"` (when in_double_quote has just been toggled back
    // off) gets pushed there.
    let ctx = ValidationContext::new(r#"jq ".foo""#);
    assert_eq!(ctx.base_command(), "jq");
    assert_eq!(ctx.fully_unquoted_pre_strip(), "jq \"");
}

// ─────────────────────────── validate_newlines now quote-aware ──────────────

#[test]
fn req_security_490_p2_1_b_newline_inside_double_quote_passes() {
    use dasclaw_bash_validation::security::{early, SecurityResult};
    // Upstream L905-L943 uses fully_unquoted_pre_strip → newlines inside
    // double quotes are stripped first, so this is Passthrough.
    let ctx = ValidationContext::new("echo \"line1\nline2\"");
    assert_eq!(early::validate_newlines(&ctx), SecurityResult::Passthrough);
}

#[test]
fn req_security_490_p2_1_b_newline_inside_single_quote_passes() {
    use dasclaw_bash_validation::security::{early, SecurityResult};
    let ctx = ValidationContext::new("echo 'line1\nline2'");
    assert_eq!(early::validate_newlines(&ctx), SecurityResult::Passthrough);
}

#[test]
fn req_security_490_p2_1_b_newline_outside_quotes_still_blocks() {
    use dasclaw_bash_validation::security::{
        early, validate_security, DecisionReason, SecurityCheckId, SecurityResult,
    };
    let ctx = ValidationContext::new("echo a\nrm -rf /");
    let result = early::validate_newlines(&ctx);
    match result {
        SecurityResult::Block {
            reason:
                DecisionReason::CommandInjection {
                    check_id, sub_id, ..
                },
        } => {
            assert_eq!(check_id, SecurityCheckId::Newlines);
            assert_eq!(sub_id, 1);
        }
        other => panic!("expected Block, got {other:?}"),
    }
    // Engine wiring sanity.
    assert!(validate_security("echo a\nrm -rf /").is_block());
}
