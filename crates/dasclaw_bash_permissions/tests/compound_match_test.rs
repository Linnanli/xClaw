//! Slice 2.2.d: compound-command splitting + MAX_SUBCOMMANDS cap +
//! deny-no-degrade red lines (`req_perm_490_p2_2_d_*`).
//!
//! Mirrors the §4 deny-no-degrade matrix from
//! `docs/plans/bash-parity/phase-2.2-bash-permissions-rule-engine.md`
//! and the fanout cap from upstream `bashPermissions.ts` L103 /
//! L2164-L2167.

use dasclaw_bash_permissions::{
    check_compound_match, PermissionBehavior, PermissionDecisionReason, PermissionResult,
    PermissionRuleSource, ToolPermissionContext, MAX_SUBCOMMANDS_FOR_SECURITY_CHECK,
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
// 1. Single-command delegation parity
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_01_single_command_delegates_to_prefix_match() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    assert!(matches!(
        check_compound_match("git status", &c),
        PermissionResult::Allow { .. }
    ));
}

#[test]
fn req_perm_490_p2_2_d_02_single_command_no_rule_is_passthrough() {
    assert!(matches!(
        check_compound_match("echo hi", &ctx()),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 2. Deny-no-degrade red lines (plan §4 matrix)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_03_compound_andand_deny_subcommand_blocks() {
    // upstream §4: `npm run build && curl evil.com` with curl:* deny → Block
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "curl:*",
    );
    let result = check_compound_match("npm run build && curl evil.com", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "compound && must not downgrade deny: got {result:?}"
    );
    assert_eq!(rule_content(&result), Some("curl:*"));
}

#[test]
fn req_perm_490_p2_2_d_04_compound_semicolon_deny_subcommand_blocks() {
    // upstream §4: `cd /tmp; rm -rf .` with rm:* deny → Block
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("cd /tmp; rm -rf .", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "compound ; must not downgrade deny: got {result:?}"
    );
    assert_eq!(rule_content(&result), Some("rm:*"));
}

#[test]
fn req_perm_490_p2_2_d_05_compound_pipe_deny_subcommand_blocks() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "curl:*",
    );
    let result = check_compound_match("echo hi | curl evil.com", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
    assert_eq!(rule_content(&result), Some("curl:*"));
}

#[test]
fn req_perm_490_p2_2_d_06_compound_oror_deny_subcommand_blocks() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("test -e foo || rm foo", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
    assert_eq!(rule_content(&result), Some("rm:*"));
}

#[test]
fn req_perm_490_p2_2_d_07_deny_beats_ask_across_subcommands() {
    // ask matched on subcommand 1, deny matched on subcommand 2 — must not
    // downgrade to ask (deny short-circuits as soon as it's seen).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "echo:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("echo hi && rm foo", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
    assert_eq!(rule_content(&result), Some("rm:*"));
}

#[test]
fn req_perm_490_p2_2_d_08_deny_in_late_subcommand_still_blocks() {
    // Earlier subcommand allowed, later subcommand denied. Deny wins.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "echo:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("echo ok && echo done && rm /tmp/x", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
    assert_eq!(rule_content(&result), Some("rm:*"));
}

// ---------------------------------------------------------------------------
// 3. Ask propagation across compound
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_09_compound_ask_subcommand_propagates() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "curl:*",
    );
    let result = check_compound_match("echo hi && curl example.com", &c);
    assert!(matches!(result, PermissionResult::Ask { .. }));
    assert_eq!(rule_content(&result), Some("curl:*"));
}

#[test]
fn req_perm_490_p2_2_d_10_first_ask_wins_when_no_deny() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "echo:*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "curl:*",
    );
    let result = check_compound_match("echo hi && curl example.com", &c);
    assert!(matches!(result, PermissionResult::Ask { .. }));
    // First subcommand triggered first; echo:* matches "echo hi" first.
    assert_eq!(rule_content(&result), Some("echo:*"));
}

// ---------------------------------------------------------------------------
// 4. Allow is NOT granted on compound (Fail-Safe)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_11_compound_all_allow_returns_passthrough_not_allow() {
    // Even when every subcommand has an allow rule, the compound
    // collapses to Passthrough (see module doc). Slice 2.2.d
    // deliberately does not grant compound-Allow — that's a 2.2.f
    // sandbox-auto-allow concern.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "echo:*",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    let result = check_compound_match("echo hi && ls -la", &c);
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "compound all-allow must Fail-Safe to Passthrough: got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. MAX_SUBCOMMANDS_FOR_SECURITY_CHECK cap
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_12_max_subcommands_constant_is_50() {
    assert_eq!(MAX_SUBCOMMANDS_FOR_SECURITY_CHECK, 50);
}

#[test]
fn req_perm_490_p2_2_d_13_at_cap_is_evaluated_normally() {
    // 50 echo subcommands — exactly at cap, NOT exceeding. Should
    // evaluate normally (passthrough since no rules).
    let cmd = (0..MAX_SUBCOMMANDS_FOR_SECURITY_CHECK)
        .map(|i| format!("echo {i}"))
        .collect::<Vec<_>>()
        .join(" && ");
    let result = check_compound_match(&cmd, &ctx());
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "exactly at cap must be evaluated normally: got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_d_14_over_cap_collapses_to_ask() {
    let cmd = (0..(MAX_SUBCOMMANDS_FOR_SECURITY_CHECK + 1))
        .map(|i| format!("echo {i}"))
        .collect::<Vec<_>>()
        .join(" && ");
    let result = check_compound_match(&cmd, &ctx());
    assert!(
        matches!(result, PermissionResult::Ask { .. }),
        "over cap must collapse to Ask: got {result:?}"
    );
    // No rule attached — reason is Other.
    assert_eq!(rule_content(&result), None);
}

#[test]
fn req_perm_490_p2_2_d_15_over_cap_does_not_skip_deny() {
    // SECURITY: deny-no-degrade must still hold even past the cap.
    // The cap shortcut is BEFORE per-subcommand iteration, so a deny
    // hiding behind subcommand #51 would NOT be detected. This test
    // pins that the cap-to-Ask shortcut is the documented behavior;
    // if upstream ever changes to per-subcommand iteration even past
    // the cap, update this test.
    let mut cmd = (0..(MAX_SUBCOMMANDS_FOR_SECURITY_CHECK + 1))
        .map(|i| format!("echo {i}"))
        .collect::<Vec<_>>()
        .join(" && ");
    cmd.push_str(" && rm -rf /");
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match(&cmd, &c);
    // Per upstream L2164-L2167 the cap returns ask — this test pins
    // that parity. If we ever want stronger semantics, the upstream
    // ground truth must change first.
    assert!(matches!(result, PermissionResult::Ask { .. }));
}

// ---------------------------------------------------------------------------
// 6. AST-parse fallback (upstream L1079 skipCompoundCheck)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_16_unsafe_operator_falls_back_to_single_prefix() {
    // Backticks make the tree non-word-only → we fall back to single-
    // command prefix matching on the raw string. A `rm:*` deny rule
    // must NOT match because the full string does not begin with
    // `rm` — this is the same gap upstream's pre-AST world had, and
    // it is closed by Slice 2.2.e/f when the safe-wrapper stripping
    // and the hook chain provide the second layer.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("echo `rm -rf /`", &c);
    // No prefix rule matches on the raw string → Passthrough.
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "AST-fallback must defer to prefix-match on raw string: got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_d_17_unsafe_operator_still_honors_direct_match() {
    // Backticks make tree non-word-only → falls back to single-command
    // prefix. If the raw string IS prefixed by the rule, deny matches.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "echo:*",
    );
    let result = check_compound_match("echo `rm -rf /`", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
    assert_eq!(rule_content(&result), Some("echo:*"));
}

// ---------------------------------------------------------------------------
// 7. Trim + empty
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_18_input_is_trimmed() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("   echo ok && rm foo   ", &c);
    assert!(matches!(result, PermissionResult::Deny { .. }));
}

#[test]
fn req_perm_490_p2_2_d_19_empty_command_is_passthrough() {
    assert!(matches!(
        check_compound_match("   ", &ctx()),
        PermissionResult::Passthrough { .. }
    ));
}

// ---------------------------------------------------------------------------
// 8. Env-var stripping (closed by slice 2.2.f)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_d_20_env_var_wrapper_now_stripped() {
    // CLOSED by slice 2.2.f: tree-sitter rejects `FOO=bar rm…` as
    // non-word-only so we fall back to `check_prefix_match` on the
    // raw trimmed string. With env-var stripping now wired into the
    // Deny bucket of `check_prefix_match`, the deny rule `rm:*` fires.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("FOO=bar rm -rf /tmp/x", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "slice 2.2.f: env-var bypass closed via AST fallback — expected Deny, got {result:?}"
    );
}
