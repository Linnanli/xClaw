//! Tests for `detect_unreachable_rules_from_context` bridge —
//! Slice 2.2.m (Issue #490 Phase 2.2).
//!
//! Integration smoke between Slice 2.2.k (context collectors) and
//! Slice 2.2.i (shadowed-rule detection). The bridge itself is a
//! 3-line composition; these tests pin that the composition
//! preserves every contract of the underlying primitives when driven
//! from a `ToolPermissionContext`.
//!
//! Test ID: `req_perm_490_p2_2_m_NN_<desc>`.

use dasclaw_bash_permissions::{
    detect_unreachable_rules_from_context,
    shadowed_rule_detection::ShadowType,
    types::{PermissionBehavior, PermissionRuleSource, ToolPermissionContext},
    DetectUnreachableRulesOptions, BASH_TOOL_NAME,
};

fn ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

const OPTS_OFF: DetectUnreachableRulesOptions = DetectUnreachableRulesOptions {
    sandbox_auto_allow_enabled: false,
};
const OPTS_ON: DetectUnreachableRulesOptions = DetectUnreachableRulesOptions {
    sandbox_auto_allow_enabled: true,
};

// -----------------------------------------------------------------
// 01 — empty context yields empty Vec (baseline Fail-Safe)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_01_empty_context() {
    let c = ctx();
    assert!(detect_unreachable_rules_from_context(&c, OPTS_OFF).is_empty());
    assert!(detect_unreachable_rules_from_context(&c, OPTS_ON).is_empty());
}

// -----------------------------------------------------------------
// 02 — context with only one bucket populated → no shadow
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_02_single_bucket_no_shadow() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    assert!(detect_unreachable_rules_from_context(&c, OPTS_OFF).is_empty());
}

// -----------------------------------------------------------------
// 03 — tool-wide Deny shadows specific Allow (end-to-end via context)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_03_tool_wide_deny_shadows_allow() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    // Tool-wide deny: `""` collapses to `rule_content: None` (parity
    // with 2.2.j parser).
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].shadow_type, ShadowType::Deny);
    assert_eq!(out[0].rule.rule_value.tool_name, BASH_TOOL_NAME);
    assert_eq!(
        out[0].rule.rule_value.rule_content.as_deref(),
        Some("git:*")
    );
    assert!(out[0].shadowed_by.rule_value.rule_content.is_none());
}

// -----------------------------------------------------------------
// 04 — tool-wide Ask shadows specific Allow when sandbox auto-allow
//      disabled
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_04_tool_wide_ask_shadows_allow_sandbox_off() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "npm install",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings, // shared source
        "*",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].shadow_type, ShadowType::Ask);
}

// -----------------------------------------------------------------
// 05 — **SECURITY PIN**: sandbox-on EXEMPTION is keyed on the *Ask*
//      rule's source (upstream L141-L148 — see Slice 2.2.i tests 08+09).
//      The bridge MUST preserve this so personal-Ask-only stays exempt.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_05_sandbox_on_exempts_personal_ask() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    // Personal Ask source (`UserSettings`) — exempt when sandbox on.
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "*",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_ON);
    assert!(
        out.is_empty(),
        "sandbox-on personal-Ask must NOT shadow specific allow: {out:?}"
    );
}

// -----------------------------------------------------------------
// 06 — **SECURITY PIN**: sandbox-on does NOT exempt a *shared* Ask
//      (upstream L141 `isSharedSettingSource(askRule.source)` check
//      — Slice 2.2.i test 11 three-way regression PIN).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_06_sandbox_on_does_not_exempt_shared_ask() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings, // shared
        "*",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_ON);
    assert_eq!(out.len(), 1, "shared-Ask must shadow even when sandbox on");
    assert_eq!(out[0].shadow_type, ShadowType::Ask);
}

// -----------------------------------------------------------------
// 07 — Deny precedence preserved: both tool-wide Ask AND Deny present
//      → Deny wins (upstream `findShadowingRule` Deny checked first).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_07_deny_precedence_over_ask() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings,
        "*",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].shadow_type, ShadowType::Deny);
}

// -----------------------------------------------------------------
// 08 — multiple Allow rules in same context, only the specific ones
//      are reported (tool-wide allow never shadowed by deny since
//      upstream short-circuits on `rule_content.is_none()`).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_08_tool_wide_allow_not_shadowed() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "", // tool-wide → rule_content: None
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "*",
    );
    let out = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].rule.rule_value.rule_content.as_deref(),
        Some("git:*"),
        "tool-wide allow must NOT be reported"
    );
}

// -----------------------------------------------------------------
// 09 — Determinism PIN: same context → identical Vec on repeated calls
//      (relies on BTreeMap source ordering + first-match-wins).
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_09_deterministic_output() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "npm:*",
    );
    c.add_rule(PermissionBehavior::Deny, PermissionRuleSource::CliArg, "");
    let a = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    let b = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
}

// -----------------------------------------------------------------
// 10 — Pure delegation PIN: bridge result MUST equal calling
//      `detect_unreachable_rules` directly with the three collectors'
//      output. Locks the "no extra logic" invariant.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_m_10_pure_delegation_invariant() {
    use dasclaw_bash_permissions::{
        detect_unreachable_rules, get_allow_rules, get_ask_rules, get_deny_rules,
    };
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "git:*",
    );
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::ProjectSettings,
        "*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::CliArg,
        "rm:*",
    );
    let via_bridge = detect_unreachable_rules_from_context(&c, OPTS_OFF);
    let manual = detect_unreachable_rules(
        &get_allow_rules(&c),
        &get_ask_rules(&c),
        &get_deny_rules(&c),
        OPTS_OFF,
    );
    assert_eq!(via_bridge, manual);
}
