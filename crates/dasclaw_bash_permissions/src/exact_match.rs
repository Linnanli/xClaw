//! Exact-match permission pipeline — port of upstream
//! `bashToolCheckExactMatchPermission` (`bashPermissions.ts` L991-L1048)
//! and its filter helper `filterRulesByContentsMatchingInput` restricted
//! to `matchMode = 'exact'` (L800-L935).
//!
//! ## Scope
//!
//! This module implements **exact-mode matching only**. Prefix /
//! wildcard mode lives in [`crate::prefix_match`] (slice 2.2.c).
//! Compound splitting (2.2.d), env-var / safe-wrapper stripping (2.2.e),
//! and hook wiring (2.2.f) land in later slices.
//!
//! ## Security posture
//!
//! - **Deny precedence**: deny rules checked first across all sources.
//!   Matches upstream L996-L1010. A deny match short-circuits — no ask
//!   / allow can override (deny-不降级 red line in plan §4).
//! - **Source-deterministic ordering**: see
//!   [`crate::pipeline::first_matching_rule`].
//! - **No env-var stripping** in this slice. A rule `Bash(rm -rf /)`
//!   will NOT match `FOO=bar rm -rf /` here — Fail-Safe for allow,
//!   Fail-Open for deny. Closed by slice 2.2.e.
//! - **Library-only** until slice 2.2.f.
//!
//! ## Upstream parity
//!
//! Exact-mode per-rule matching from upstream L872-895:
//!
//! - `Exact { command }` → matches iff `rule.command == cmd_to_match`
//! - `Prefix { prefix }` → matches iff `rule.prefix == cmd_to_match`
//!   (NOT `startsWith` — that is prefix *mode*, in
//!   [`crate::prefix_match`])
//! - `Wildcard { .. }` → **always false** in exact mode (upstream L920:
//!   "wildcards must NOT match because we're checking the full unparsed
//!   command. Wildcard matching on unparsed commands allows `foo *` to
//!   match `foo arg && curl evil.com` since `.*` matches operators.")

use crate::pipeline::{run_pipeline, PipelineMessages, RulePredicate};
use crate::shell_rule_matching::{parse_permission_rule, ShellPermissionRule};
use crate::types::{PermissionResult, ToolPermissionContext};

/// Bash tool name — separated as a const so future tool-agnostic
/// refactors (slice 2.2.f) can override.
pub const BASH_TOOL_NAME: &str = "Bash";

/// Per-rule predicate for `matchMode = 'exact'`.
///
/// Direct port of the inner `commandsToTry.some(...)` predicate from
/// upstream `filterRulesByContentsMatchingInput` (L870-L935), specialised
/// to `matchMode === 'exact'`.
fn rule_matches_in_exact_mode(rule_content: &str, cmd_to_match: &str) -> bool {
    match parse_permission_rule(rule_content) {
        ShellPermissionRule::Exact { command } => command == cmd_to_match,
        ShellPermissionRule::Prefix { prefix } => prefix == cmd_to_match,
        ShellPermissionRule::Wildcard { .. } => false,
    }
}

const EXACT_PREDICATE: RulePredicate = rule_matches_in_exact_mode;

/// Run the exact-match permission pipeline against a bash command.
///
/// Returns a [`PermissionResult`] following upstream precedence
/// `Deny > Ask > Allow > Passthrough` (upstream L996-L1048).
///
/// `command` is trimmed before matching, mirroring upstream L995
/// (`const command = input.command.trim()`). No env-var stripping or
/// compound-splitting happens here — see slice 2.2.d/e.
pub fn check_exact_match(command: &str, context: &ToolPermissionContext) -> PermissionResult {
    let trimmed = command.trim();
    run_pipeline(
        trimmed,
        &|_behavior| trimmed.to_string(),
        context,
        BASH_TOOL_NAME,
        EXACT_PREDICATE,
        PipelineMessages {
            deny: &|cmd| {
                format!(
                    "Permission to use {tool} with command {cmd} has been denied.",
                    tool = BASH_TOOL_NAME,
                )
            },
            ask: &|| {
                format!(
                    "Claude requested permissions to use {tool}, but you haven't granted it yet.",
                    tool = BASH_TOOL_NAME,
                )
            },
            passthrough: &|| "This command requires approval".to_string(),
        },
    )
}
