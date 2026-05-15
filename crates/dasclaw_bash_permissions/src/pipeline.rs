//! Shared rule-matching pipeline used by both `exact_match` and
//! `prefix_match` modes.
//!
//! Extracts the source-deterministic iteration over a
//! [`ToolPermissionRulesBySource`] and the Deny>Ask>Allow>Passthrough
//! precedence loop. Each match mode supplies its own per-rule predicate
//! and the wrapping permission-decision messages; everything else is
//! shared.
//!
//! This module is `pub(crate)`: callers go through
//! [`crate::exact_match::check_exact_match`] or
//! [`crate::prefix_match::check_prefix_match`].

use crate::types::{
    PermissionBehavior, PermissionDecisionReason, PermissionResult, PermissionRule,
    PermissionRuleValue, ToolPermissionContext, ToolPermissionRulesBySource,
};

/// Per-rule predicate signature: given a rule's stored content string
/// and the trimmed command, return whether the rule matches.
///
/// Modes differ only in this predicate. See:
/// - [`crate::exact_match`] for `matchMode = 'exact'`
/// - [`crate::prefix_match`] for `matchMode = 'prefix'`
pub(crate) type RulePredicate = fn(rule_content: &str, cmd_to_match: &str) -> bool;

/// Scan a [`ToolPermissionRulesBySource`] and return the first rule
/// whose content satisfies `predicate`, preserving (source, content)
/// provenance for audit logs.
///
/// Iteration order is deterministic:
/// 1. sources in [`crate::types::PermissionRuleSource`] enum order
///    (via `BTreeMap` keys)
/// 2. insertion order within each source's `Vec<String>`
///
/// This determinism is the audit-log `rule_id` reproducibility contract
/// pinned by tests in `bash_security_audit_log.rs` (#524 / #530 pattern).
pub(crate) fn first_matching_rule(
    rules_by_source: &ToolPermissionRulesBySource,
    cmd_to_match: &str,
    behavior: PermissionBehavior,
    tool_name: &str,
    predicate: RulePredicate,
) -> Option<PermissionRule> {
    for (source, rule_contents) in rules_by_source {
        for rule_content in rule_contents {
            if predicate(rule_content, cmd_to_match) {
                return Some(PermissionRule {
                    source: *source,
                    rule_behavior: behavior,
                    rule_value: PermissionRuleValue {
                        tool_name: tool_name.to_string(),
                        rule_content: Some(rule_content.clone()),
                    },
                });
            }
        }
    }
    None
}

/// Run the Deny > Ask > Allow > Passthrough pipeline against a trimmed
/// command, parameterised by a per-rule predicate.
///
/// This mirrors upstream `bashToolCheckExactMatchPermission`
/// (L996-L1048) and `bashToolCheckPermission` (L1050+) precedence,
/// which is shared verbatim between both match modes. Both upstream
/// entry points feed into `matchingRulesForInput` (L937-L985) with
/// different `matchMode` values; the precedence loop is identical.
///
/// `deny_message` / `ask_message` / `passthrough_message` are mode- and
/// tool-specific (e.g. exact mode wants "with command X has been
/// denied" while prefix mode may want a different phrasing in later
/// slices). For now both modes use the same upstream wording.
pub(crate) fn run_pipeline(
    command: &str,
    context: &ToolPermissionContext,
    tool_name: &str,
    predicate: RulePredicate,
    deny_message: &dyn Fn(&str) -> String,
    ask_message: &dyn Fn() -> String,
    passthrough_message: &dyn Fn() -> String,
) -> PermissionResult {
    let cmd = command.trim();

    if let Some(rule) = first_matching_rule(
        &context.always_deny_rules,
        cmd,
        PermissionBehavior::Deny,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Deny {
            message: deny_message(cmd),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    if let Some(rule) = first_matching_rule(
        &context.always_ask_rules,
        cmd,
        PermissionBehavior::Ask,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Ask {
            message: ask_message(),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    if let Some(rule) = first_matching_rule(
        &context.always_allow_rules,
        cmd,
        PermissionBehavior::Allow,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Allow {
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    let passthrough = passthrough_message();
    PermissionResult::Passthrough {
        message: passthrough.clone(),
        reason: PermissionDecisionReason::Other {
            reason: passthrough,
        },
    }
}
