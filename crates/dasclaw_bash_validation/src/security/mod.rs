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
mod types;

pub use ast::{BashAst, ParseFail};
pub use context::ValidationContext;
pub use quote_extract::{
    extract_quoted_content, has_unescaped_char, strip_safe_redirections, QuoteExtraction,
};
pub use types::{DecisionReason, SecurityCheckId, SecurityResult};

/// Public entry point for the Phase 2.1 security gate.
///
/// Runs the four early validators sequentially; first non-Passthrough result
/// wins.
///
/// **Slice 2.1.a behaviour**: only the four early validators run. Slice 2.1.b
/// will splice in the deferred-non-misparsing engine (upstream
/// `bashSecurity.ts` L2371-2387) and the 19 main validators.
pub fn validate_security(command: &str) -> SecurityResult {
    let ctx = ValidationContext::new(command);

    let validators: &[fn(&ValidationContext) -> SecurityResult] = &[
        early::validate_empty,
        early::validate_incomplete_commands,
        early::validate_newlines,
        early::validate_carriage_return,
        early::validate_unicode_whitespace,
    ];

    for validator in validators {
        match validator(&ctx) {
            SecurityResult::Passthrough => continue,
            decisive => return decisive,
        }
    }

    SecurityResult::Passthrough
}
