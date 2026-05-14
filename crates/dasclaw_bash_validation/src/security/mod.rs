//! Bash command-injection security gate (Phase 2.1 port of upstream
//! `claude-code-main/src/tools/BashTool/bashSecurity.ts`).
//!
//! See [`docs/plans/bash-parity/phase-2.1-bash-security-alignment.md`] for the
//! semantic-port contract, deviations from the upstream TypeScript, and the
//! anti-drift strategy (asymmetric invariant + differential CI).
//!
//! ## Implemented so far
//!
//! - Slice 2.1.a — early validators + AST wrapper:
//!   - [`SecurityCheckId`] / [`SecurityResult`] / [`DecisionReason`]
//!   - [`ast::parse_for_security`] Fail-Closed AST wrapper
//!   - [`early`] — 5 early validators (empty / incomplete / newlines / CR /
//!     unicode whitespace)
//! - Slice 2.1.b — context & quote infrastructure:
//!   - [`ValidationContext`] now carries the 5 derived views upstream uses
//!   - [`quote_extract`] ports `extractQuotedContent`,
//!     `stripSafeRedirections`, `hasUnescapedChar`
//!   - AST is parsed once and cached on the context
//!   - [`early::validate_newlines`] switched to `fully_unquoted_pre_strip`
//!     to match upstream (no false positive on `echo "line1\nline2"`)
//!
//! Slice 2.1.c will add the 19 main validators + the deferred-non-misparsing
//! engine. Slice 2.1.d wires the hook + e2e tests.
//!
//! ## Asymmetric invariant
//!
//! ```text
//! ∀ command c:
//!   upstream_blocks(c) ⟹ xclaw_blocks(c)        // false negatives forbidden
//!   xclaw_blocks(c)    ⟹⁄ upstream_blocks(c)   // we may be stricter
//! ```

pub mod ast;
pub mod context;
pub mod early;
pub mod quote_extract;
pub mod regex_validators;
mod types;

pub use ast::{BashAst, ParseFail};
pub use context::ValidationContext;
pub use quote_extract::{
    extract_quoted_content, has_unescaped_char, strip_safe_redirections, QuoteExtraction,
};
pub use types::{DecisionReason, SecurityCheckId, SecurityResult};

/// Public entry point for the Phase 2.1 security gate.
///
/// Runs validators in upstream `bashSecurity.ts` order (L2348-L2377),
/// preserving the **deferred-non-misparsing engine** (L2393-L2407):
///
/// - Validators flagged by [`SecurityCheckId::is_misparsing`] return
///   immediately on the first non-Passthrough result.
/// - Validators flagged as non-misparsing (`validate_newlines`,
///   `validate_redirections`) have their decisive result *deferred*: a later
///   misparsing-sensitive validator that fires gets priority. If no
///   misparsing validator fires, the earliest deferred non-misparsing
///   result is surfaced.
///
/// In Phase 2.1 we collapse `ask` → `Block` regardless, but preserving the
/// deferred ordering means the surfaced [`DecisionReason`] mirrors upstream's
/// precedence — a precondition for the differential test invariant.
pub fn validate_security(command: &str) -> SecurityResult {
    let ctx = ValidationContext::new(command);

    // Upstream pipeline order. Each entry: (validator, check_id_for_misparsing_flag).
    //
    // The check_id stored here is only used to consult `is_misparsing` — the
    // validator itself populates the real check_id inside `DecisionReason`.
    //
    // NOTE: Slice 2.1.c1 lands only the 9 regex-only validators below
    // (plus the 5 early ones). Slice 2.1.c2 will splice in the 9 AST-walking
    // validators (jq*, obfuscated, backslash*, brace, zsh, malformed) at
    // their upstream positions. Order matters for `DecisionReason`
    // precedence; entries are placed at their final upstream slots so c2
    // is a pure insertion.
    type V = fn(&ValidationContext) -> SecurityResult;
    let pipeline: &[(V, SecurityCheckId)] = &[
        // --- Early phase (always misparsing-sensitive in our port) ---
        (early::validate_empty, SecurityCheckId::IncompleteCommands),
        (
            early::validate_incomplete_commands,
            SecurityCheckId::IncompleteCommands,
        ),
        // (Slice 2.1.c2) validate_jq_*, validate_obfuscated_flags
        (
            regex_validators::validate_shell_metacharacters,
            SecurityCheckId::ShellMetacharacters,
        ),
        (
            regex_validators::validate_dangerous_variables,
            SecurityCheckId::DangerousVariables,
        ),
        (
            regex_validators::validate_comment_quote_desync,
            SecurityCheckId::CommentQuoteDesync,
        ),
        (
            regex_validators::validate_quoted_newline,
            SecurityCheckId::QuotedNewline,
        ),
        (
            early::validate_carriage_return,
            SecurityCheckId::IncompleteCommands,
        ),
        // validate_newlines is non-misparsing → deferred.
        (early::validate_newlines, SecurityCheckId::Newlines),
        (
            regex_validators::validate_ifs_injection,
            SecurityCheckId::IfsInjection,
        ),
        (
            regex_validators::validate_proc_environ_access,
            SecurityCheckId::ProcEnvironAccess,
        ),
        (
            regex_validators::validate_dangerous_patterns,
            SecurityCheckId::DangerousPatternsCommandSubstitution,
        ),
        // validate_redirections is non-misparsing → deferred.
        (
            regex_validators::validate_redirections,
            SecurityCheckId::DangerousPatternsInputRedirection,
        ),
        // (Slice 2.1.c2) validate_backslash_escaped_whitespace,
        //                validate_backslash_escaped_operators
        (
            early::validate_unicode_whitespace,
            SecurityCheckId::UnicodeWhitespace,
        ),
        (
            regex_validators::validate_mid_word_hash,
            SecurityCheckId::MidWordHash,
        ),
        // (Slice 2.1.c2) validate_brace_expansion, validate_zsh_dangerous_commands,
        //                validate_malformed_token_injection
    ];

    let mut deferred: Option<SecurityResult> = None;

    for (validator, flag_id) in pipeline {
        match validator(&ctx) {
            SecurityResult::Passthrough => continue,
            SecurityResult::Allow => return SecurityResult::Allow,
            decisive @ SecurityResult::Block { .. } => {
                if flag_id.is_misparsing() {
                    return decisive;
                }
                // Non-misparsing: defer (keep earliest only).
                if deferred.is_none() {
                    deferred = Some(decisive);
                }
            }
        }
    }

    deferred.unwrap_or(SecurityResult::Passthrough)
}
