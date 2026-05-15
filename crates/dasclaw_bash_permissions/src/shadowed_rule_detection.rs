//! Shadowed (unreachable) permission rule detection — verbatim port of
//! upstream `claude-code-main/src/utils/permissions/shadowedRuleDetection.ts`
//! L1-L234 plus the `permissionRuleSourceDisplayString` helper from
//! `permissions.ts` L116-L120 and `getSettingSourceDisplayNameLowercase`
//! from `utils/settings/constants.ts` L72-L93.
//!
//! ## Scope
//!
//! This slice (2.2.i) ports the **pure analyzer** that, given three
//! pre-collected rule lists, identifies allow rules made unreachable by
//! tool-wide deny/ask rules of the same tool. It is intentionally
//! **input-driven** — callers pass `Vec<PermissionRule>` collected from
//! their own settings layer. We do NOT port `getAllowRules` /
//! `getAskRules` / `getDenyRules` here because our crate's
//! [`crate::types::ToolPermissionContext`] is bash-only and stores
//! parsed inner content strings (not the upstream
//! `"Bash(content)" | "Bash"` raw form); the upstream collectors are
//! storage-shape-dependent, the analyzer is not.
//!
//! ## Issue #490 red-line check
//!
//! - Algorithm L1-L234 ported verbatim; comments map shape-for-shape
//! - Bash sandbox-auto-allow exception preserved (upstream L107-L114,
//!   L141-L150), including the "ask-rule's-source matters, NOT allow's"
//!   subtlety — pinned by `req_perm_490_p2_2_i_*` tests.
//! - No upstream ANT-only code in this file
//! - No new crate deps; pure Rust over existing
//!   [`crate::types::PermissionRule`].

use crate::exact_match::BASH_TOOL_NAME;
use crate::types::{PermissionBehavior, PermissionRule, PermissionRuleSource};

/// Type of shadowing that makes a rule unreachable. Mirrors upstream
/// `ShadowType = 'ask' | 'deny'` (L14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowType {
    Ask,
    Deny,
}

/// An unreachable permission rule with explanation. Mirrors upstream
/// `UnreachableRule` (L18-L24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreachableRule {
    pub rule: PermissionRule,
    pub reason: String,
    pub shadowed_by: PermissionRule,
    pub shadow_type: ShadowType,
    pub fix: String,
}

/// Options for detecting unreachable rules. Mirrors upstream
/// `DetectUnreachableRulesOptions` (L28-L36).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetectUnreachableRulesOptions {
    /// Whether sandbox auto-allow is enabled for Bash commands. When
    /// true, tool-wide Bash ask rules from **personal** settings don't
    /// block specific Bash allow rules (sandboxed commands are
    /// auto-allowed).
    pub sandbox_auto_allow_enabled: bool,
}

/// Result of checking if a rule is shadowed. Discriminated union for
/// type safety — mirrors upstream `ShadowResult` (L41-L43).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ShadowResult {
    NotShadowed,
    Shadowed {
        shadowed_by: PermissionRule,
        shadow_type: ShadowType,
    },
}

/// Check if a permission rule source is **shared** (visible to other
/// users). Verbatim port of upstream `isSharedSettingSource` (L60-L66).
///
/// Shared:
/// - `ProjectSettings` (committed to git, shared with team)
/// - `PolicySettings` (enterprise-managed, pushed to all users)
/// - `Command` (from slash command frontmatter, potentially shared)
///
/// Personal:
/// - `UserSettings`, `LocalSettings`, `CliArg`, `Session`, `FlagSettings`
pub fn is_shared_setting_source(source: PermissionRuleSource) -> bool {
    matches!(
        source,
        PermissionRuleSource::ProjectSettings
            | PermissionRuleSource::PolicySettings
            | PermissionRuleSource::Command
    )
}

/// Format a rule source for display in warning messages. Lowercase form,
/// verbatim port of upstream `getSettingSourceDisplayNameLowercase`
/// (`utils/settings/constants.ts` L72-L93).
pub fn permission_rule_source_display_string(source: PermissionRuleSource) -> &'static str {
    match source {
        PermissionRuleSource::UserSettings => "user settings",
        PermissionRuleSource::ProjectSettings => "shared project settings",
        PermissionRuleSource::LocalSettings => "project local settings",
        PermissionRuleSource::FlagSettings => "command line arguments",
        PermissionRuleSource::PolicySettings => "enterprise managed settings",
        PermissionRuleSource::CliArg => "CLI argument",
        PermissionRuleSource::Command => "command configuration",
        PermissionRuleSource::Session => "current session",
    }
}

/// Generate a fix suggestion based on the shadow type. Verbatim port of
/// upstream `generateFixSuggestion` (L77-L92).
fn generate_fix_suggestion(
    shadow_type: ShadowType,
    shadowing_rule: &PermissionRule,
    shadowed_rule: &PermissionRule,
) -> String {
    let shadowing_source = permission_rule_source_display_string(shadowing_rule.source);
    let shadowed_source = permission_rule_source_display_string(shadowed_rule.source);
    let tool_name = &shadowing_rule.rule_value.tool_name;
    match shadow_type {
        ShadowType::Deny => format!(
            "Remove the \"{tool_name}\" deny rule from {shadowing_source}, or remove the specific allow rule from {shadowed_source}"
        ),
        ShadowType::Ask => format!(
            "Remove the \"{tool_name}\" ask rule from {shadowing_source}, or remove the specific allow rule from {shadowed_source}"
        ),
    }
}

/// Check if a specific allow rule is shadowed (unreachable) by an
/// **ask** rule. Verbatim port of `isAllowRuleShadowedByAskRule`
/// (L116-L153).
///
/// An allow rule is unreachable when:
/// 1. There's a tool-wide ask rule (e.g. `Bash` in ask list)
/// 2. AND a specific allow rule (e.g. `Bash(ls:*)` in allow list)
///
/// Exception: for Bash with sandbox auto-allow enabled, tool-wide ask
/// rules from **personal** settings don't shadow specific allow rules
/// because sandboxed commands are auto-allowed regardless. The exception
/// is keyed on the **ask** rule's source, not the allow rule's
/// (upstream L141-L148) — Issue #490 SECURITY pin (`req_perm_490_p2_2_i_*`).
fn is_allow_rule_shadowed_by_ask_rule(
    allow_rule: &PermissionRule,
    ask_rules: &[PermissionRule],
    options: DetectUnreachableRulesOptions,
) -> ShadowResult {
    let tool_name = &allow_rule.rule_value.tool_name;
    let rule_content = allow_rule.rule_value.rule_content.as_ref();

    // Only check allow rules that have specific content (e.g.,
    // `Bash(ls:*)`). Tool-wide allow rules cannot be shadowed by ask
    // rules. (Upstream L122-L125.)
    if rule_content.is_none() {
        return ShadowResult::NotShadowed;
    }

    // Find any tool-wide ask rule for the same tool.
    let Some(shadowing_ask_rule) = ask_rules
        .iter()
        .find(|r| &r.rule_value.tool_name == tool_name && r.rule_value.rule_content.is_none())
    else {
        return ShadowResult::NotShadowed;
    };

    // Special case: Bash with sandbox auto-allow from personal settings.
    // (Upstream L141-L148.) Verbatim nested-if shape preserved to match
    // the upstream control flow comment "Fall through to mark as
    // shadowed - shared settings should always warn".
    #[allow(
        clippy::collapsible_if,
        reason = "verbatim upstream control flow L141-L148"
    )]
    if tool_name == BASH_TOOL_NAME && options.sandbox_auto_allow_enabled {
        if !is_shared_setting_source(shadowing_ask_rule.source) {
            return ShadowResult::NotShadowed;
        }
        // Fall through — shared settings should always warn.
    }

    ShadowResult::Shadowed {
        shadowed_by: shadowing_ask_rule.clone(),
        shadow_type: ShadowType::Ask,
    }
}

/// Check if an allow rule is shadowed (completely blocked) by a **deny**
/// rule. Verbatim port of `isAllowRuleShadowedByDenyRule` (L166-L188).
///
/// More severe than ask shadowing — deny rules short-circuit the
/// pipeline.
fn is_allow_rule_shadowed_by_deny_rule(
    allow_rule: &PermissionRule,
    deny_rules: &[PermissionRule],
) -> ShadowResult {
    let tool_name = &allow_rule.rule_value.tool_name;
    let rule_content = allow_rule.rule_value.rule_content.as_ref();

    // Only check allow rules that have specific content. (Upstream
    // L173-L176.)
    if rule_content.is_none() {
        return ShadowResult::NotShadowed;
    }

    let Some(shadowing_deny_rule) = deny_rules
        .iter()
        .find(|r| &r.rule_value.tool_name == tool_name && r.rule_value.rule_content.is_none())
    else {
        return ShadowResult::NotShadowed;
    };

    ShadowResult::Shadowed {
        shadowed_by: shadowing_deny_rule.clone(),
        shadow_type: ShadowType::Deny,
    }
}

/// Detect all unreachable permission rules given the pre-collected
/// allow / ask / deny rule lists. Verbatim port of
/// `detectUnreachableRules` (L194-L233).
///
/// Currently detects:
/// - Allow rules shadowed by tool-wide deny rules (more severe —
///   completely blocked)
/// - Allow rules shadowed by tool-wide ask rules (will always prompt)
///
/// **Caller responsibility**: pass only rules where `rule_behavior`
/// matches the list slot (`Allow` in `allow_rules`, etc.). The function
/// does not re-validate — that matches upstream where the three lists
/// come from typed accessors.
pub fn detect_unreachable_rules(
    allow_rules: &[PermissionRule],
    ask_rules: &[PermissionRule],
    deny_rules: &[PermissionRule],
    options: DetectUnreachableRulesOptions,
) -> Vec<UnreachableRule> {
    let mut unreachable: Vec<UnreachableRule> = Vec::new();

    // (Upstream L204-L232.)
    for allow_rule in allow_rules {
        // Guard against caller passing a non-allow rule into the allow
        // slot — silently skip rather than panic (Fail-Safe).
        if allow_rule.rule_behavior != PermissionBehavior::Allow {
            continue;
        }

        // Check deny shadowing first (more severe). (Upstream L206-L217.)
        let deny_result = is_allow_rule_shadowed_by_deny_rule(allow_rule, deny_rules);
        if let ShadowResult::Shadowed {
            shadowed_by,
            shadow_type,
        } = deny_result
        {
            let shadow_source = permission_rule_source_display_string(shadowed_by.source);
            let reason = format!(
                "Blocked by \"{tool}\" deny rule (from {shadow_source})",
                tool = shadowed_by.rule_value.tool_name,
            );
            let fix = generate_fix_suggestion(shadow_type, &shadowed_by, allow_rule);
            unreachable.push(UnreachableRule {
                rule: allow_rule.clone(),
                reason,
                shadowed_by,
                shadow_type,
                fix,
            });
            continue; // Don't also report ask-shadowing if deny-shadowed.
        }

        // Check ask shadowing. (Upstream L219-L231.)
        let ask_result = is_allow_rule_shadowed_by_ask_rule(allow_rule, ask_rules, options);
        if let ShadowResult::Shadowed {
            shadowed_by,
            shadow_type,
        } = ask_result
        {
            let shadow_source = permission_rule_source_display_string(shadowed_by.source);
            let reason = format!(
                "Shadowed by \"{tool}\" ask rule (from {shadow_source})",
                tool = shadowed_by.rule_value.tool_name,
            );
            let fix = generate_fix_suggestion(shadow_type, &shadowed_by, allow_rule);
            unreachable.push(UnreachableRule {
                rule: allow_rule.clone(),
                reason,
                shadowed_by,
                shadow_type,
                fix,
            });
        }
    }

    unreachable
}
