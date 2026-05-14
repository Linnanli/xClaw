//! Exact-match permission pipeline — port of upstream
//! `bashToolCheckExactMatchPermission` (`bashPermissions.ts` L991-L1048)
//! and its filter helper `filterRulesByContentsMatchingInput` restricted
//! to `matchMode = 'exact'` (L800-L935).
//!
//! ## Scope (Slice 2.2.b)
//!
//! This slice implements **exact-match only**. Prefix / wildcard match
//! modes (prefix-mode in upstream parlance — `startsWith`, `xargs <p>`
//! word-boundary, compound-command splitting, env-var stripping) land
//! in subsequent slices per plan §5:
//!
//! - Slice 2.2.c → prefix / wildcard match
//! - Slice 2.2.d → compound operator splitting + `MAX_SUBCOMMANDS_FOR_SECURITY_CHECK`
//! - Slice 2.2.e → `stripSafeWrappers` + `stripAllLeadingEnvVars` + `BINARY_HIJACK_VARS`
//! - Slice 2.2.f → `BashPermissionHook` impl + `CompositeSafetyHook` wiring
//!
//! ## Security posture
//!
//! - **Deny precedence**: deny rules checked first across all sources.
//!   Matches upstream L996-L1010. A deny match short-circuits — no ask /
//!   allow can override (deny-不降级 red line in plan §4).
//! - **Source-deterministic ordering**: rules are iterated in
//!   [`PermissionRuleSource`] enum order via `BTreeMap` keys, then in
//!   insertion order within each source. This makes `rule_id` audit
//!   attribution reproducible across runs (regression target for the
//!   audit-log pinning pattern established in #524 / #530).
//! - **Library-only until 2.2.f**: nothing in this slice is wired into
//!   `CompositeSafetyHook`. The engine is callable from tests + future
//!   slices but cannot affect runtime behavior yet.
//! - **No env-var stripping** in this slice. A rule `Bash(npm:*)` will
//!   NOT match `FOO=bar npm test` here — that is Fail-Safe for *allow*
//!   rules (won't auto-allow a wrapped command) but Fail-Open for *deny*
//!   rules. Documented as a known gap closed by slice 2.2.e.
//!
//! ## Upstream parity
//!
//! Exact-mode per-rule matching from upstream L872-895:
//!
//! - `Exact { command }` → matches iff `rule.command == cmd_to_match`
//! - `Prefix { prefix }` → matches iff `rule.prefix == cmd_to_match`
//!   (NOT `startsWith` — that's prefix *mode*, a different parameter)
//! - `Wildcard { .. }` → **always false** in exact mode (upstream L920:
//!   "wildcards must NOT match because we're checking the full unparsed
//!   command. Wildcard matching on unparsed commands allows `foo *` to
//!   match `foo arg && curl evil.com` since `.*` matches operators.")

use crate::shell_rule_matching::{parse_permission_rule, ShellPermissionRule};
use crate::types::{
    PermissionBehavior, PermissionDecisionReason, PermissionResult, PermissionRule,
    PermissionRuleValue, ToolPermissionContext, ToolPermissionRulesBySource,
};

/// Bash tool name — separated as a const so future tool-agnostic
/// refactors (slice 2.2.f) can override.
pub const BASH_TOOL_NAME: &str = "Bash";

/// Determine whether `rule_content` matches `cmd_to_match` in **exact mode**.
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

/// Scan a `ToolPermissionRulesBySource` and return the first matching
/// rule (if any), preserving (source, content) provenance for audit logs.
///
/// Returns `None` if no rule matches. Iteration order is deterministic:
/// sources in [`PermissionRuleSource`] enum order (via `BTreeMap`), then
/// insertion order within each source's `Vec<String>`.
fn first_matching_rule(
    rules_by_source: &ToolPermissionRulesBySource,
    cmd_to_match: &str,
    behavior: PermissionBehavior,
) -> Option<PermissionRule> {
    for (source, rule_contents) in rules_by_source {
        for rule_content in rule_contents {
            if rule_matches_in_exact_mode(rule_content, cmd_to_match) {
                return Some(PermissionRule {
                    source: *source,
                    rule_behavior: behavior,
                    rule_value: PermissionRuleValue {
                        tool_name: BASH_TOOL_NAME.to_string(),
                        rule_content: Some(rule_content.clone()),
                    },
                });
            }
        }
    }
    None
}

/// Run the exact-match permission pipeline against a bash command.
///
/// Returns a [`PermissionResult`] following upstream precedence
/// `Deny > Ask > Allow > Passthrough` (upstream L996-L1048).
///
/// `command` is trimmed before matching, mirroring upstream L995
/// (`const command = input.command.trim()`). No env-var stripping or
/// compound-splitting happens here — see slice 2.2.d/e.
pub fn check_exact_match(command: &str, context: &ToolPermissionContext) -> PermissionResult {
    let cmd = command.trim();

    // 1. Deny rules — checked first across all sources. A match here
    //    short-circuits regardless of any ask / allow rule. Matches the
    //    "deny-不降级" red line in plan §4.
    if let Some(rule) =
        first_matching_rule(&context.always_deny_rules, cmd, PermissionBehavior::Deny)
    {
        return PermissionResult::Deny {
            message: format!(
                "Permission to use {tool} with command {cmd} has been denied.",
                tool = BASH_TOOL_NAME,
            ),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    // 2. Ask rules — matched commands require user approval.
    if let Some(rule) = first_matching_rule(&context.always_ask_rules, cmd, PermissionBehavior::Ask)
    {
        return PermissionResult::Ask {
            message: format!(
                "Claude requested permissions to use {tool}, but you haven't granted it yet.",
                tool = BASH_TOOL_NAME,
            ),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    // 3. Allow rules — explicit user grants.
    if let Some(rule) =
        first_matching_rule(&context.always_allow_rules, cmd, PermissionBehavior::Allow)
    {
        return PermissionResult::Allow {
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    // 4. Passthrough — no engine-level rule fired. Caller (slice 2.2.f
    //    hook adapter) decides next: prompt user, run classifier, etc.
    PermissionResult::Passthrough {
        message: "This command requires approval".to_string(),
        reason: PermissionDecisionReason::Other {
            reason: "This command requires approval".to_string(),
        },
    }
}
