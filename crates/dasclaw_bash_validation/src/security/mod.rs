//! Bash command-injection security gate (Phase 2.1 port of upstream
//! `claude-code-main/src/tools/BashTool/bashSecurity.ts`).
//!
//! See [`docs/plans/bash-parity/phase-2.1-bash-security-alignment.md`] for the
//! semantic-port contract, deviations from the upstream TypeScript, and the
//! anti-drift strategy (asymmetric invariant + differential CI).
//!
//! ## Slice 2.1.a scope
//!
//! This module currently implements:
//! - [`SecurityCheckId`] — strongly-typed enumeration of all 24 check IDs
//! - [`SecurityResult`] — port of upstream `PermissionResult` (semantic; not 1:1)
//! - [`ValidationContext`] — context passed to each validator
//! - [`ast::parse_for_security`] — Fail-Closed AST wrapper (re-uses
//!   `dasclaw_shell_command::bash::try_parse_shell`)
//! - [`early`] — 4 early validators: incomplete commands, newlines (LF),
//!   carriage return (with quote state machine), unicode whitespace
//!
//! Phase 2.1.b will add the 19 main validators + deferred-non-misparsing
//! engine. Phase 2.1.c wires the hook + e2e tests.
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
mod types;

pub use ast::{BashAst, ParseFail};
pub use context::ValidationContext;
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
