//! Prefix / wildcard permission pipeline — port of upstream
//! `bashToolCheckPermission` (`bashPermissions.ts` L1050+) inner loop,
//! restricted to `matchMode = 'prefix'` and the per-rule predicate
//! within `filterRulesByContentsMatchingInput` (L800-L935).
//!
//! ## Scope (Slice 2.2.c)
//!
//! Adds prefix-mode rule matching on top of the shared
//! [`crate::pipeline`]:
//!
//! - `Exact { command }` → equality (same as exact mode)
//! - `Prefix { prefix }` → word-boundary `startsWith` **or** `xargs`
//!   form (upstream L893-L912)
//! - `Wildcard { pattern }` → [`match_wildcard_pattern`] (upstream
//!   L926-L932)
//!
//! ## Deferred to later slices (NOT in this PR)
//!
//! These upstream behaviors are intentionally **not** included here.
//! Each gap is documented in tests so the slice that closes it has an
//! explicit forward-pointer.
//!
//! - Slice 2.2.d → compound-command splitting + `isCompoundCommand`
//!   guard (upstream L894-L896 / L915-L919). Without it, a prefix rule
//!   `Bash(cd:*)` will currently match `cd /path && rm -rf /` in this
//!   library-only engine. Pinned by [`tests/prefix_match_test.rs`]
//!   test `req_perm_490_p2_2_c_18_compound_not_yet_split`.
//! - Slice 2.2.e → `stripSafeWrappers` / `stripAllLeadingEnvVars` /
//!   `BINARY_HIJACK_VARS` (upstream L805-L856). Pinned by
//!   `req_perm_490_p2_2_c_19_env_var_wrapping_not_yet_stripped`.
//! - Output-redirection stripping (`extractOutputRedirections`,
//!   upstream L791-L801) — applies to both modes; deferred follow-up.
//! - Slice 2.2.f → `BashPermissionHook` adapter + `CompositeSafetyHook`
//!   wiring. Until then this module is library-only — the deferred
//!   gaps cannot affect runtime safety.
//!
//! ## Security posture
//!
//! - **Deny precedence**: shared with exact mode via
//!   [`crate::pipeline::run_pipeline`].
//! - **Word boundary**: prefix `ls` MUST NOT match `lsof` or `lsattr`.
//!   Enforced by requiring `prefix == cmd` or `cmd.starts_with(prefix + ' ')`.
//! - **`xargs <prefix>` form**: upstream allows `Bash(grep:*)` to match
//!   `xargs grep pattern` so that `xargs` doesn't become a bypass for
//!   deny rules. Bare `xargs <prefix>` only — flagged invocations like
//!   `xargs -n1 grep` are NOT matched (natural word boundary).
//! - **Library-only** until slice 2.2.f.

use crate::pipeline::{run_pipeline, RulePredicate};
use crate::shell_rule_matching::{
    match_wildcard_pattern, parse_permission_rule, MatchOptions, ShellPermissionRule,
};
use crate::types::{PermissionResult, ToolPermissionContext};

/// Bash tool name — same as [`crate::exact_match::BASH_TOOL_NAME`],
/// re-declared here to keep the prefix module independently usable.
pub const BASH_TOOL_NAME: &str = "Bash";

/// Per-rule predicate for `matchMode = 'prefix'`.
///
/// Direct port of upstream L872-L932 predicate, specialised to
/// `matchMode === 'prefix'`. **Compound-command guard NOT applied**
/// here — see module-level docs. The `Wildcard` branch reuses
/// [`match_wildcard_pattern`] which already implements upstream's
/// escape handling + trailing-wildcard optionalization.
fn rule_matches_in_prefix_mode(rule_content: &str, cmd_to_match: &str) -> bool {
    match parse_permission_rule(rule_content) {
        ShellPermissionRule::Exact { command } => command == cmd_to_match,
        ShellPermissionRule::Prefix { prefix } => prefix_with_word_boundary(&prefix, cmd_to_match),
        ShellPermissionRule::Wildcard { pattern } => {
            match_wildcard_pattern(&pattern, cmd_to_match, MatchOptions::default())
        }
    }
}

/// Word-boundary prefix match — port of upstream L897-L912.
///
/// Returns true iff one of:
///
/// 1. `cmd == prefix` (e.g. rule `Bash(ls:*)` matches bare `ls`)
/// 2. `cmd` starts with `prefix + ' '` (e.g. `Bash(ls:*)` matches
///    `ls -la`)
/// 3. `cmd == "xargs " + prefix` (e.g. `Bash(grep:*)` matches bare
///    `xargs grep`)
/// 4. `cmd` starts with `"xargs " + prefix + ' '` (e.g. `Bash(grep:*)`
///    matches `xargs grep pattern`)
///
/// The space-suffix guard ensures `ls` does NOT match `lsof` / `lsattr`.
fn prefix_with_word_boundary(prefix: &str, cmd: &str) -> bool {
    if prefix == cmd {
        return true;
    }
    if let Some(rest) = cmd.strip_prefix(prefix) {
        if rest.starts_with(' ') {
            return true;
        }
    }
    // xargs <prefix> form (upstream L908-L912)
    if let Some(after) = cmd.strip_prefix("xargs ") {
        if after == prefix {
            return true;
        }
        if let Some(rest) = after.strip_prefix(prefix) {
            if rest.starts_with(' ') {
                return true;
            }
        }
    }
    false
}

const PREFIX_PREDICATE: RulePredicate = rule_matches_in_prefix_mode;

/// Run the prefix-match permission pipeline against a bash command.
///
/// Returns a [`PermissionResult`] following upstream precedence
/// `Deny > Ask > Allow > Passthrough` (shared with exact mode via
/// [`crate::pipeline::run_pipeline`]).
///
/// `command` is trimmed before matching. No env-var stripping,
/// safe-wrapper stripping, or compound-command splitting happens here
/// — see module docs.
pub fn check_prefix_match(command: &str, context: &ToolPermissionContext) -> PermissionResult {
    run_pipeline(
        command,
        context,
        BASH_TOOL_NAME,
        PREFIX_PREDICATE,
        &|cmd| {
            format!(
                "Permission to use {tool} with command {cmd} has been denied.",
                tool = BASH_TOOL_NAME,
            )
        },
        &|| {
            format!(
                "Claude requested permissions to use {tool}, but you haven't granted it yet.",
                tool = BASH_TOOL_NAME,
            )
        },
        &|| "This command requires approval".to_string(),
    )
}
