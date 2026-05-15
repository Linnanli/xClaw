//! Slice 2.2.i — `req_perm_490_p2_2_i_*` request tests for
//! `shadowed_rule_detection`. Verifies verbatim parity with upstream
//! `claude-code-main/src/utils/permissions/shadowedRuleDetection.ts`
//! L1-L234.

use dasclaw_bash_permissions::shadowed_rule_detection::{
    detect_unreachable_rules, is_shared_setting_source, permission_rule_source_display_string,
    DetectUnreachableRulesOptions, ShadowType, UnreachableRule,
};
use dasclaw_bash_permissions::types::{
    PermissionBehavior, PermissionRule, PermissionRuleSource, PermissionRuleValue,
};
use dasclaw_bash_permissions::BASH_TOOL_NAME;

fn rule(
    behavior: PermissionBehavior,
    source: PermissionRuleSource,
    tool: &str,
    content: Option<&str>,
) -> PermissionRule {
    PermissionRule {
        source,
        rule_behavior: behavior,
        rule_value: PermissionRuleValue {
            tool_name: tool.to_string(),
            rule_content: content.map(str::to_string),
        },
    }
}

fn allow(source: PermissionRuleSource, tool: &str, content: Option<&str>) -> PermissionRule {
    rule(PermissionBehavior::Allow, source, tool, content)
}
fn ask(source: PermissionRuleSource, tool: &str, content: Option<&str>) -> PermissionRule {
    rule(PermissionBehavior::Ask, source, tool, content)
}
fn deny(source: PermissionRuleSource, tool: &str, content: Option<&str>) -> PermissionRule {
    rule(PermissionBehavior::Deny, source, tool, content)
}

const OPTS_OFF: DetectUnreachableRulesOptions = DetectUnreachableRulesOptions {
    sandbox_auto_allow_enabled: false,
};
const OPTS_ON: DetectUnreachableRulesOptions = DetectUnreachableRulesOptions {
    sandbox_auto_allow_enabled: true,
};

// ---------------------------------------------------------------------------
// 01 — empty inputs → empty output (baseline)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_01_empty_input_returns_empty() {
    let res = detect_unreachable_rules(&[], &[], &[], OPTS_OFF);
    assert!(res.is_empty());
}

// ---------------------------------------------------------------------------
// 02 — tool-wide allow rule NEVER reported as shadowed
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_02_tool_wide_allow_not_shadowed() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let asks = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &asks, &denies, OPTS_OFF);
    assert!(res.is_empty(), "tool-wide allow can never be shadowed");
}

// ---------------------------------------------------------------------------
// 03 — specific allow shadowed by tool-wide deny → reported with deny
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_03_specific_allow_shadowed_by_tool_wide_deny() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(res.len(), 1);
    let u: &UnreachableRule = &res[0];
    assert_eq!(u.shadow_type, ShadowType::Deny);
    assert!(u.reason.contains("Blocked by"));
    assert!(u.reason.contains("Bash"));
    assert!(u.reason.contains("shared project settings"));
    assert!(u.fix.contains("deny rule"));
    assert!(u.fix.contains("shared project settings"));
    assert!(u.fix.contains("user settings"));
}

// ---------------------------------------------------------------------------
// 04 — specific allow shadowed by tool-wide ask → reported with ask
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_04_specific_allow_shadowed_by_tool_wide_ask() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let asks = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_OFF);
    assert_eq!(res.len(), 1);
    let u: &UnreachableRule = &res[0];
    assert_eq!(u.shadow_type, ShadowType::Ask);
    assert!(u.reason.contains("Shadowed by"));
    assert!(u.reason.contains("Bash"));
    assert!(u.fix.contains("ask rule"));
}

// ---------------------------------------------------------------------------
// 05 — deny takes precedence over ask (upstream L208 PIN)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_05_deny_takes_precedence_over_ask() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let asks = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &asks, &denies, OPTS_OFF);
    assert_eq!(
        res.len(),
        1,
        "deny-shadowed should not ALSO be reported ask-shadowed"
    );
    assert_eq!(res[0].shadow_type, ShadowType::Deny);
}

// ---------------------------------------------------------------------------
// 06 — different tool tool-wide rule does NOT shadow Bash specific
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_06_different_tool_does_not_shadow() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let denies = vec![deny(PermissionRuleSource::UserSettings, "Edit", None)];
    let asks = vec![ask(PermissionRuleSource::UserSettings, "Read", None)];
    let res = detect_unreachable_rules(&allows, &asks, &denies, OPTS_OFF);
    assert!(res.is_empty());
}

// ---------------------------------------------------------------------------
// 07 — specific deny / specific ask does NOT shadow (only tool-wide does)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_07_specific_shadowing_rule_does_not_shadow() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let specific_deny = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        Some("rm:*"),
    )];
    let specific_ask = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        Some("npm:*"),
    )];
    let res = detect_unreachable_rules(&allows, &specific_ask, &specific_deny, OPTS_OFF);
    assert!(
        res.is_empty(),
        "specific same-tool deny/ask must NOT shadow specific allow"
    );
}

// ---------------------------------------------------------------------------
// 08 — Bash sandbox auto-allow ON + ask from PERSONAL settings → NOT shadowed
//      (upstream L141-L148 SECURITY pin — exception keyed on ASK rule's source)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_08_bash_sandbox_personal_ask_not_shadowed() {
    let personal_sources = [
        PermissionRuleSource::UserSettings,
        PermissionRuleSource::LocalSettings,
        PermissionRuleSource::CliArg,
        PermissionRuleSource::Session,
        PermissionRuleSource::FlagSettings,
    ];
    for src in personal_sources {
        let allows = vec![allow(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("ls:*"),
        )];
        let asks = vec![ask(src, BASH_TOOL_NAME, None)];
        let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_ON);
        assert!(
            res.is_empty(),
            "Bash sandbox auto-allow ON + personal ask source {src:?} must NOT shadow"
        );
    }
}

// ---------------------------------------------------------------------------
// 09 — Bash sandbox auto-allow ON + ask from SHARED settings → STILL shadowed
//      (upstream L146-L148 — shared settings always warn so other team
//      members without sandbox aren't surprised)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_09_bash_sandbox_shared_ask_still_shadowed() {
    let shared_sources = [
        PermissionRuleSource::ProjectSettings,
        PermissionRuleSource::PolicySettings,
        PermissionRuleSource::Command,
    ];
    for src in shared_sources {
        let allows = vec![allow(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("ls:*"),
        )];
        let asks = vec![ask(src, BASH_TOOL_NAME, None)];
        let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_ON);
        assert_eq!(
            res.len(),
            1,
            "Bash sandbox ON + shared ask source {src:?} must STILL shadow"
        );
        assert_eq!(res[0].shadow_type, ShadowType::Ask);
    }
}

// ---------------------------------------------------------------------------
// 10 — Bash sandbox auto-allow ON exception applies ONLY to Bash
//      (non-Bash tool: still shadowed by personal ask)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_10_sandbox_exception_only_for_bash() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        "Edit",
        Some("path:*"),
    )];
    let asks = vec![ask(PermissionRuleSource::UserSettings, "Edit", None)];
    let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_ON);
    assert_eq!(
        res.len(),
        1,
        "Edit tool must NOT benefit from sandbox auto-allow"
    );
}

// ---------------------------------------------------------------------------
// 11 — sandbox exception is keyed on ASK rule's source NOT allow's
//      (upstream L141-L148 SECURITY subtlety PIN)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_11_sandbox_keyed_on_ask_source_not_allow() {
    // Allow from SHARED source, ASK from PERSONAL source, sandbox ON,
    // Bash: NOT shadowed (because ask is personal).
    let allows = vec![allow(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let asks = vec![ask(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_ON);
    assert!(
        res.is_empty(),
        "exception keyed on ASK source (personal) regardless of allow's source"
    );

    // Inverse: allow from PERSONAL, ask from SHARED → SHADOWED.
    let allows2 = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let asks2 = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res2 = detect_unreachable_rules(&allows2, &asks2, &[], OPTS_ON);
    assert_eq!(
        res2.len(),
        1,
        "ask=shared → shadowed even if allow is personal"
    );
}

// ---------------------------------------------------------------------------
// 12 — sandbox OFF → personal ask STILL shadows (default path)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_12_sandbox_off_personal_ask_shadows() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let asks = vec![ask(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &asks, &[], OPTS_OFF);
    assert_eq!(res.len(), 1, "sandbox OFF: personal ask still shadows");
    assert_eq!(res[0].shadow_type, ShadowType::Ask);
}

// ---------------------------------------------------------------------------
// 13 — is_shared_setting_source matrix (upstream L60-L66)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_13_is_shared_setting_source_matrix() {
    let shared = [
        PermissionRuleSource::ProjectSettings,
        PermissionRuleSource::PolicySettings,
        PermissionRuleSource::Command,
    ];
    let personal = [
        PermissionRuleSource::UserSettings,
        PermissionRuleSource::LocalSettings,
        PermissionRuleSource::CliArg,
        PermissionRuleSource::Session,
        PermissionRuleSource::FlagSettings,
    ];
    for s in shared {
        assert!(is_shared_setting_source(s), "{s:?} must be shared");
    }
    for p in personal {
        assert!(
            !is_shared_setting_source(p),
            "{p:?} must NOT be shared (personal)"
        );
    }
}

// ---------------------------------------------------------------------------
// 14 — display string for every source (upstream constants.ts L72-L93)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_14_display_string_for_every_source() {
    let cases = [
        (PermissionRuleSource::UserSettings, "user settings"),
        (
            PermissionRuleSource::ProjectSettings,
            "shared project settings",
        ),
        (
            PermissionRuleSource::LocalSettings,
            "project local settings",
        ),
        (PermissionRuleSource::FlagSettings, "command line arguments"),
        (
            PermissionRuleSource::PolicySettings,
            "enterprise managed settings",
        ),
        (PermissionRuleSource::CliArg, "CLI argument"),
        (PermissionRuleSource::Command, "command configuration"),
        (PermissionRuleSource::Session, "current session"),
    ];
    for (src, expected) in cases {
        assert_eq!(
            permission_rule_source_display_string(src),
            expected,
            "display string for {src:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 15 — multiple specific allow rules all reported (loop iteration PIN)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_15_multiple_specific_allows_all_reported() {
    let allows = vec![
        allow(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("ls:*"),
        ),
        allow(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("git status"),
        ),
        allow(
            PermissionRuleSource::LocalSettings,
            BASH_TOOL_NAME,
            Some("npm test"),
        ),
    ];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(res.len(), 3);
    for u in &res {
        assert_eq!(u.shadow_type, ShadowType::Deny);
    }
}

// ---------------------------------------------------------------------------
// 16 — non-allow rules in `allow_rules` slot silently skipped (Fail-Safe)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_16_misplaced_rule_in_allow_slot_skipped() {
    // Deliberately put a Deny rule in the allow_rules slot — should be
    // skipped, not panic, not reported.
    let allows = vec![
        deny(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("ls:*"),
        ),
        allow(
            PermissionRuleSource::UserSettings,
            BASH_TOOL_NAME,
            Some("git:*"),
        ),
    ];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(res.len(), 1, "only the real Allow rule is reported");
    assert_eq!(
        res[0].rule.rule_value.rule_content.as_deref(),
        Some("git:*")
    );
}

// ---------------------------------------------------------------------------
// 17 — first matching shadowing rule wins (find semantics PIN, upstream L132/L181)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_17_first_matching_shadowing_wins() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let denies = vec![
        deny(PermissionRuleSource::ProjectSettings, BASH_TOOL_NAME, None),
        deny(PermissionRuleSource::PolicySettings, BASH_TOOL_NAME, None),
    ];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(res.len(), 1);
    assert_eq!(
        res[0].shadowed_by.source,
        PermissionRuleSource::ProjectSettings,
        "first deny in the list wins (upstream find() semantics)"
    );
}

// ---------------------------------------------------------------------------
// 18 — fix suggestion mentions both shadowing and shadowed sources
//      (upstream L86-L91 format pin)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_18_fix_suggestion_format() {
    let allows = vec![allow(
        PermissionRuleSource::LocalSettings,
        BASH_TOOL_NAME,
        Some("npm test"),
    )];
    let denies = vec![deny(
        PermissionRuleSource::PolicySettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(res.len(), 1);
    assert!(res[0].fix.contains("enterprise managed settings"));
    assert!(res[0].fix.contains("project local settings"));
    assert!(res[0].fix.contains("\"Bash\" deny rule"));
}

// ---------------------------------------------------------------------------
// 19 — when allow rule has rule_content=Some(""), still treated as specific
//      (only `None` is tool-wide per upstream `ruleContent === undefined`)
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_19_empty_string_content_treated_as_specific() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some(""),
    )];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        None,
    )];
    let res = detect_unreachable_rules(&allows, &[], &denies, OPTS_OFF);
    assert_eq!(
        res.len(),
        1,
        "Some(\"\") is NOT None — still a specific allow"
    );
}

// ---------------------------------------------------------------------------
// 20 — deny + ask with both specific (NOT tool-wide) leaves allow safe
// ---------------------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_i_20_no_tool_wide_shadowing_rule_no_report() {
    let allows = vec![allow(
        PermissionRuleSource::UserSettings,
        BASH_TOOL_NAME,
        Some("ls:*"),
    )];
    let denies = vec![deny(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        Some("rm:*"),
    )];
    let asks = vec![ask(
        PermissionRuleSource::ProjectSettings,
        BASH_TOOL_NAME,
        Some("git:*"),
    )];
    assert!(detect_unreachable_rules(&allows, &asks, &denies, OPTS_OFF).is_empty());
    assert!(detect_unreachable_rules(&allows, &asks, &denies, OPTS_ON).is_empty());
}
