//! [`ValidationContext`] — read-only bundle handed to every security validator.
//!
//! Slice 2.1.b populates the full set of pre-computed views that upstream
//! `bashSecurity.ts` (L103-L196) hands to its validators:
//!
//! - `original_command` — exact input bytes
//! - `base_command` — `command.split(' ')[0]` (jq detection, etc.)
//! - `unquoted_content` — content with `'…'` stripped but `"…"` kept (and
//!   safe redirections stripped)
//! - `fully_unquoted_content` — content with `'…'` and `"…"` stripped AND
//!   safe redirections stripped
//! - `fully_unquoted_pre_strip` — same as above **before** safe redirections
//!   are stripped (used by `validate_newlines` / `validate_brace_expansion`
//!   to avoid stripping-induced false negatives)
//! - `unquoted_keep_quote_chars` — quoted content stripped but quote
//!   **delimiters** preserved (used by `validate_mid_word_hash` to spot
//!   `'x'#` adjacencies)
//!
//! The AST is parsed **eagerly** (once) at construction time and cached as
//! `Result<BashAst, ParseFail>` so the deferred engine in Slice 2.1.c can
//! map parse failures into [`crate::security::DecisionReason::ParseFailure`]
//! without re-parsing.
//!
//! Heredoc body stripping (upstream `extractHeredocs` L2287-L2289) is
//! intentionally **deferred to Slice 2.1.c** — see plan §5.3. Slice 2.1.b
//! treats `processed_command == original_command`. The validators that rely
//! on `fully_unquoted_*` therefore behave identically to upstream on every
//! input *except* commands containing quoted heredocs (`<<'EOF'` / `<<\EOF`).

use super::ast::{parse_for_security, BashAst, ParseFail};
use super::quote_extract::{extract_quoted_content, strip_safe_redirections, QuoteExtraction};

/// Read-only bundle of pre-computed views of the command. Constructed once
/// per `validate_security` call and shared (by reference) with every
/// validator.
#[derive(Debug)]
pub struct ValidationContext<'a> {
    original_command: &'a str,
    base_command: String,
    unquoted_content: String,
    fully_unquoted_content: String,
    fully_unquoted_pre_strip: String,
    unquoted_keep_quote_chars: String,
    ast: Result<BashAst, ParseFail>,
}

impl<'a> ValidationContext<'a> {
    /// Build a context for `command` by running the upstream quote / strip
    /// pipeline and eagerly parsing the AST.
    ///
    /// Mirrors upstream `bashSecurity.ts` L2287-L2306 minus the heredoc
    /// pre-pass (deferred to Slice 2.1.c).
    pub fn new(command: &'a str) -> Self {
        // Upstream L2295: `command.split(' ')[0] || ''`. Note this is
        // *space*-only split — not whitespace — because base-command
        // detection is tolerant of leading tabs being a separate Block-able
        // signal (see `validate_incomplete_commands` sub 1).
        let base_command = command.split(' ').next().unwrap_or("").to_string();

        let is_jq = base_command == "jq";
        let QuoteExtraction {
            with_double_quotes,
            fully_unquoted,
            unquoted_keep_quote_chars,
        } = extract_quoted_content(command, is_jq);

        let fully_unquoted_content = strip_safe_redirections(&fully_unquoted);
        let ast = parse_for_security(command);

        Self {
            original_command: command,
            base_command,
            unquoted_content: with_double_quotes,
            fully_unquoted_content,
            fully_unquoted_pre_strip: fully_unquoted,
            unquoted_keep_quote_chars,
            ast,
        }
    }

    pub fn original_command(&self) -> &str {
        self.original_command
    }

    pub fn base_command(&self) -> &str {
        &self.base_command
    }

    pub fn unquoted_content(&self) -> &str {
        &self.unquoted_content
    }

    pub fn fully_unquoted_content(&self) -> &str {
        &self.fully_unquoted_content
    }

    pub fn fully_unquoted_pre_strip(&self) -> &str {
        &self.fully_unquoted_pre_strip
    }

    pub fn unquoted_keep_quote_chars(&self) -> &str {
        &self.unquoted_keep_quote_chars
    }

    /// Cached AST. `Ok(&BashAst)` on success, `Err(&ParseFail)` if
    /// tree-sitter could not produce an ERROR-free tree.
    ///
    /// Slice 2.1.c's deferred engine maps `Err(&ParseFail)` to
    /// [`crate::security::DecisionReason::ParseFailure`] (Fail-Closed).
    pub fn ast(&self) -> Result<&BashAst, &ParseFail> {
        self.ast.as_ref()
    }
}
