//! Slice 2.2.c: prefix / wildcard match pipeline tests
//! (`req_perm_490_p2_2_c_*`).
//!
//! Verbatim semantic parity with upstream
//! `bashToolCheckPermission` → `matchingRulesForInput('prefix')` →
//! `filterRulesByContentsMatchingInput(..., 'prefix')` predicate, minus
//! features explicitly deferred to slices 2.2.d/e (compound splitting,
//! env-var / safe-wrapper stripping).
//!
//! Deferred-gap forward pointers:
//! - test 18 → slice 2.2.d (compound splitting)
//! - test 19 → **CLOSED in slice 2.2.f** (env-var stripping wired into
//!   `check_prefix_match`); assertion flipped from Passthrough → Deny.

use dasclaw_bash_permissions::{
    check_prefix_match, PermissionBehavior, PermissionDecisionReason, PermissionResult,
    PermissionRuleSource, ToolPermissionContext,
};

fn ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

fn rule_content(result: &PermissionResult) -> Option<&str> {
    let reason = match result {
        PermissionResult::Deny { reason, .. }
        | PermissionResult::Ask { reason, .. }
        | PermissionResult::Allow { reason }
        | PermissionResult::Passthrough { reason, .. } => reason,
    };
    match reason {
        PermissionDecisionReason::Rule { rule } => rule.rule_value.rule_content.as_deref(),
        PermissionDecisionReason::Other { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// 1. Exact-shape rules still work in prefix mode (parity baseline)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_01_exact_rule_strict_equality() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git status",
    );
    assert!(matches!(
        check_prefix_match("git status", &c),
        PermissionResult::Allow { .. }
    ));
    assert!(matches!(
        check_prefix_match("git status --short", &c),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 2. Prefix rule (legacy `name:*` syntax) — word boundary semantics
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_02_prefix_rule_bare_command_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    assert!(matches!(
        check_prefix_match("ls", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_03_prefix_rule_with_args_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    assert!(matches!(
        check_prefix_match("ls -la /tmp", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_04_prefix_rule_word_boundary_rejects_substring() {
    // SECURITY: `ls:*` MUST NOT match `lsof` / `lsattr` — without the
    // space-boundary guard, plain startsWith would let `ls:*` grant
    // access to unrelated `ls`-prefixed binaries (upstream L897-L905).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    assert!(matches!(
        check_prefix_match("lsof", &c),
        PermissionResult::Passthrough { .. }
    ));
    assert!(matches!(
        check_prefix_match("lsattr -d /", &c),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 3. xargs <prefix> form (upstream L908-L912)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_05_xargs_prefix_bare_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "grep:*",
    );
    assert!(matches!(
        check_prefix_match("xargs grep", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_06_xargs_prefix_with_args_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "grep:*",
    );
    assert!(matches!(
        check_prefix_match("xargs grep pattern file.txt", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_07_xargs_flagged_invocation_not_matched() {
    // Natural word-boundary: `xargs -n1 grep` does NOT start with
    // `xargs grep ` so flagged xargs invocations are NOT matched
    // (upstream comment L910-L912).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "grep:*",
    );
    assert!(matches!(
        check_prefix_match("xargs -n1 grep pattern", &c),
        PermissionResult::Passthrough { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_08_xargs_deny_blocks_circumvention() {
    // Deny side: `Bash(rm:*)` must block `xargs rm file` so xargs
    // doesn't become a deny-rule bypass (upstream comment L908).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "rm:*",
    );
    assert!(matches!(
        check_prefix_match("xargs rm /tmp/junk", &c),
        PermissionResult::Deny { .. }
    ));
}

// ---------------------------------------------------------------------------
// 4. Wildcard rule — uses `match_wildcard_pattern` (slice 2.2.a port)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_09_wildcard_rule_matches_pattern() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git diff *",
    );
    assert!(matches!(
        check_prefix_match("git diff HEAD~1", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_10_wildcard_rule_trailing_optionalization() {
    // `git *` matches both `git add` and bare `git` per upstream
    // trailing-wildcard optionalization (shell_rule_matching docs).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git *",
    );
    assert!(matches!(
        check_prefix_match("git", &c),
        PermissionResult::Allow { .. }
    ));
    assert!(matches!(
        check_prefix_match("git status", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_11_wildcard_rule_no_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git diff *",
    );
    assert!(matches!(
        check_prefix_match("git status", &c),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 5. Precedence (Deny > Ask > Allow > Passthrough) — same loop as exact mode
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_12_deny_overrides_allow_in_prefix_mode() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    assert!(matches!(
        check_prefix_match("rm -rf /tmp/foo", &c),
        PermissionResult::Deny { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_13_ask_overrides_allow_in_prefix_mode() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings,
        "ls:*",
    );
    assert!(matches!(
        check_prefix_match("ls -la", &c),
        PermissionResult::Ask { .. }
    ));
}

// ---------------------------------------------------------------------------
// 6. Trim + provenance + source determinism (shared with exact mode)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_14_command_trimming() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    assert!(matches!(
        check_prefix_match("   ls -la   ", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_c_15_source_determinism_user_before_project() {
    let mut c = ctx();
    // Add in non-canonical order to ensure BTreeMap re-orders.
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "ls:*",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    let result = check_prefix_match("ls -la", &c);
    let reason = match &result {
        PermissionResult::Allow { reason } => reason,
        _ => panic!("expected Allow"),
    };
    match reason {
        PermissionDecisionReason::Rule { rule } => {
            assert_eq!(rule.source, PermissionRuleSource::UserSettings);
        }
        _ => panic!("expected rule reason"),
    }
}

#[test]
fn req_perm_490_p2_2_c_16_provenance_carries_rule_content() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("rm -rf /", &c);
    assert_eq!(rule_content(&result), Some("rm:*"));
}

#[test]
fn req_perm_490_p2_2_c_17_empty_context_is_passthrough() {
    assert!(matches!(
        check_prefix_match("anything goes here", &ctx()),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 7. Known gaps — forward pointers to slices 2.2.d and 2.2.e
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_c_18_compound_not_yet_split() {
    // GAP: slice 2.2.d will add compound-command splitting +
    // `isCompoundCommand` guard (upstream L894-L896 / L915-L919).
    // Until then, `Bash(cd:*)` will still match
    // `cd /path && rm -rf /` here because the engine sees the whole
    // string as one command starting with `cd `. This is library-only
    // and not wired anywhere; slice 2.2.d MUST flip this to
    // Passthrough (and the matching deny-side test in 2.2.d MUST keep
    // `Bash(rm:*)` matching `cd /path && rm -rf /` so denies survive).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "cd:*",
    );
    let result = check_prefix_match("cd /path && rm -rf /", &c);
    assert!(
        matches!(result, PermissionResult::Allow { .. }),
        "current behavior pinned for forward-pointer; slice 2.2.d \
         MUST change this to Passthrough"
    );
}

#[test]
fn req_perm_490_p2_2_c_19_env_var_wrapping_now_stripped() {
    // CLOSED by slice 2.2.f: `stripAllLeadingEnvVars` is wired into
    // the Deny bucket so `FOO=bar denied_command` no longer bypasses
    // deny rules. Upstream `permissions.ts` L805-L856.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=bar rm -rf /", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "slice 2.2.f: env-var bypass closed — expected Deny, got {result:?}"
    );
}
