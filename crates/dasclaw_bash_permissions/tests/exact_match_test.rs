//! Slice 2.2.b — `check_exact_match` tests.
//!
//! Naming family: `req_perm_490_p2_2_b_<id>_<desc>`.
//!
//! Coverage targets (mapped to plan §6 + upstream `bashPermissions.ts`
//! L991-L1048):
//!
//! - precedence `Deny > Ask > Allow > Passthrough`
//! - all 3 [`ShellPermissionRule`] variants in exact mode:
//!   `Exact` matches, `Prefix` matches only on equality (not startsWith),
//!   `Wildcard` never matches
//! - command trimming (leading/trailing whitespace)
//! - empty / no-match → Passthrough with `Other` reason
//! - rule attribution: returned [`PermissionRule`] carries the rule
//!   content + source + behavior that fired
//! - deterministic source-iteration order

use dasclaw_bash_permissions::{
    check_exact_match, PermissionBehavior, PermissionDecisionReason, PermissionResult,
    PermissionRuleSource, ToolPermissionContext, BASH_TOOL_NAME,
};

// ---------- helpers ----------

fn ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

fn assert_deny_with_rule_content(result: &PermissionResult, expected: &str) {
    match result {
        PermissionResult::Deny { reason, .. } => match reason {
            PermissionDecisionReason::Rule { rule } => {
                assert_eq!(rule.rule_value.tool_name, BASH_TOOL_NAME);
                assert_eq!(rule.rule_value.rule_content.as_deref(), Some(expected));
                assert_eq!(rule.rule_behavior, PermissionBehavior::Deny);
            }
            other => panic!("expected Rule reason, got {other:?}"),
        },
        other => panic!("expected Deny, got {other:?}"),
    }
}

fn assert_allow_with_rule_content(result: &PermissionResult, expected: &str) {
    match result {
        PermissionResult::Allow { reason } => match reason {
            PermissionDecisionReason::Rule { rule } => {
                assert_eq!(rule.rule_value.rule_content.as_deref(), Some(expected));
                assert_eq!(rule.rule_behavior, PermissionBehavior::Allow);
            }
            other => panic!("expected Rule reason, got {other:?}"),
        },
        other => panic!("expected Allow, got {other:?}"),
    }
}

fn assert_ask_with_rule_content(result: &PermissionResult, expected: &str) {
    match result {
        PermissionResult::Ask { reason, .. } => match reason {
            PermissionDecisionReason::Rule { rule } => {
                assert_eq!(rule.rule_value.rule_content.as_deref(), Some(expected));
                assert_eq!(rule.rule_behavior, PermissionBehavior::Ask);
            }
            other => panic!("expected Rule reason, got {other:?}"),
        },
        other => panic!("expected Ask, got {other:?}"),
    }
}

// ---------- precedence ----------

#[test]
fn req_perm_490_p2_2_b_01_deny_short_circuits_allow() {
    // Deny precedence — even if an allow rule matches the exact same
    // command, deny wins. Mirrors upstream L996-L1010 + plan §4 red line.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    assert_deny_with_rule_content(&check_exact_match("rm -rf /", &c), "rm -rf /");
}

#[test]
fn req_perm_490_p2_2_b_02_deny_short_circuits_ask() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "curl evil.com",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings,
        "curl evil.com",
    );
    assert_deny_with_rule_content(&check_exact_match("curl evil.com", &c), "curl evil.com");
}

#[test]
fn req_perm_490_p2_2_b_03_ask_short_circuits_allow() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "git push",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git push",
    );
    assert_ask_with_rule_content(&check_exact_match("git push", &c), "git push");
}

#[test]
fn req_perm_490_p2_2_b_04_passthrough_when_no_rule_matches() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls",
    );
    match check_exact_match("pwd", &c) {
        PermissionResult::Passthrough { reason, .. } => match reason {
            PermissionDecisionReason::Other { .. } => {}
            other => panic!("expected Other reason on Passthrough, got {other:?}"),
        },
        other => panic!("expected Passthrough, got {other:?}"),
    }
}

// ---------- rule-shape matching in exact mode ----------

#[test]
fn req_perm_490_p2_2_b_05_exact_rule_matches_exact_command() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git status",
    );
    assert_allow_with_rule_content(&check_exact_match("git status", &c), "git status");
}

#[test]
fn req_perm_490_p2_2_b_06_exact_rule_does_not_match_substring() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git status",
    );
    assert!(matches!(
        check_exact_match("git status --short", &c),
        PermissionResult::Passthrough { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_b_07_prefix_rule_matches_only_on_equality_in_exact_mode() {
    // Upstream L876-L878: in exact mode, a Prefix rule matches iff
    // `rule.prefix === cmdToMatch`. It must NOT do startsWith — that's
    // prefix MODE (different parameter, lands in slice 2.2.c).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    // Equals the bare prefix → match
    assert_allow_with_rule_content(&check_exact_match("git", &c), "git:*");
    // startsWith → must NOT match in exact mode
    assert!(matches!(
        check_exact_match("git status", &c),
        PermissionResult::Passthrough { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_b_08_wildcard_rule_never_matches_in_exact_mode() {
    // Upstream L920-L925 SECURITY FIX: in exact mode, wildcards MUST NOT
    // match because `.*` would otherwise allow operator-injection like
    // `foo arg && curl evil.com` against rule `foo *`.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "foo *",
    );
    // Even a pure benign extension must not be allowed in exact mode.
    assert!(matches!(
        check_exact_match("foo arg", &c),
        PermissionResult::Passthrough { .. }
    ));
    // The operator-injection variant must also Passthrough (Fail-Safe).
    assert!(matches!(
        check_exact_match("foo arg && curl evil.com", &c),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------- command normalization ----------

#[test]
fn req_perm_490_p2_2_b_09_command_is_trimmed_before_matching() {
    // Upstream L995: `const command = input.command.trim()`. Leading /
    // trailing whitespace must not defeat an exact rule.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    assert_deny_with_rule_content(&check_exact_match("   rm -rf /   ", &c), "rm -rf /");
    assert_deny_with_rule_content(&check_exact_match("\trm -rf /\n", &c), "rm -rf /");
}

// ---------- multi-source determinism ----------

#[test]
fn req_perm_490_p2_2_b_10_source_iteration_is_deterministic() {
    // Two deny rules in different sources, both match. The first match
    // returned must always come from the source that sorts earlier in
    // `PermissionRuleSource` enum order (UserSettings < ProjectSettings).
    // This pins the audit-log `rule_id` reproducibility guarantee.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "rm -rf /",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    match check_exact_match("rm -rf /", &c) {
        PermissionResult::Deny { reason, .. } => match reason {
            PermissionDecisionReason::Rule { rule } => {
                assert_eq!(
                    rule.source,
                    PermissionRuleSource::UserSettings,
                    "first-matching-rule must follow PermissionRuleSource enum order"
                );
            }
            other => panic!("expected Rule reason, got {other:?}"),
        },
        other => panic!("expected Deny, got {other:?}"),
    }
}

#[test]
fn req_perm_490_p2_2_b_11_rule_provenance_carries_source_and_behavior() {
    // Audit log + UI need (source, behavior, content) on every rule-driven
    // result. This pins all 3 across all 3 behaviors.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::PolicySettings,
        "docker run",
    );
    match check_exact_match("docker run", &c) {
        PermissionResult::Ask { reason, .. } => match reason {
            PermissionDecisionReason::Rule { rule } => {
                assert_eq!(rule.source, PermissionRuleSource::PolicySettings);
                assert_eq!(rule.rule_behavior, PermissionBehavior::Ask);
                assert_eq!(rule.rule_value.rule_content.as_deref(), Some("docker run"));
                assert_eq!(rule.rule_value.tool_name, "Bash");
            }
            other => panic!("expected Rule reason, got {other:?}"),
        },
        other => panic!("expected Ask, got {other:?}"),
    }
}

// ---------- empty contexts ----------

#[test]
fn req_perm_490_p2_2_b_12_empty_context_returns_passthrough() {
    // No rules of any kind → Passthrough with Other reason. This is the
    // baseline state when a fresh process starts with no settings loaded.
    let c = ctx();
    assert!(matches!(
        check_exact_match("ls -la", &c),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------- known-gap pinning (deferred to 2.2.e) ----------

#[test]
fn req_perm_490_p2_2_b_13_env_var_wrapping_not_yet_stripped() {
    // Documented known gap: this slice does NOT strip leading env var
    // assignments. `FOO=bar rm -rf /` is NOT (yet) recognised as
    // equivalent to `rm -rf /` — that recognition lands in slice 2.2.e
    // (`stripAllLeadingEnvVars`). Pin the current behavior so 2.2.e
    // landing flips this assertion intentionally (red test prompts the
    // 2.2.e PR to update this test).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm -rf /",
    );
    assert!(
        matches!(
            check_exact_match("FOO=bar rm -rf /", &c),
            PermissionResult::Passthrough { .. }
        ),
        "PRE-2.2.e: env-var-wrapped command does not match bare-command rule. \
         When slice 2.2.e lands `strip_all_leading_env_vars`, flip this to Deny."
    );
}
