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
/// Per-behavior command resolver: returns the command string to match
/// against rules of the given behavior bucket.
///
/// Modes that need different stripping per behavior (e.g. prefix mode
/// uses [`crate::strip_env::strip_all_leading_env_vars`] on the Deny
/// path and [`crate::strip_env::strip_safe_wrappers`] on the
/// Allow / Ask path — upstream `permissions.ts` L805-L856) supply a
/// closure that switches on `behavior`. Modes that don't (e.g. exact
/// mode) return the same trimmed string regardless.
///
/// The `&str` returned must live for the duration of one pipeline call.
/// The trait-object form (`&dyn Fn`) is used to keep the API
/// monomorphisation-free and avoids leaking the closure type into the
/// public re-exports of `exact_match` / `prefix_match`.
pub(crate) type CmdForBehavior<'a> = &'a dyn Fn(PermissionBehavior) -> String;

/// Mode-/tool-specific decision messages bundled into one struct so
/// [`run_pipeline`] stays under clippy's `too_many_arguments` cap.
/// Each closure is invoked at most once per call.
pub(crate) struct PipelineMessages<'a> {
    /// Invoked when a Deny rule matches; receives the original
    /// (display) command — never the post-stripped form.
    pub deny: &'a dyn Fn(&str) -> String,
    /// Invoked when an Ask rule matches.
    pub ask: &'a dyn Fn() -> String,
    /// Invoked when no rule matches.
    pub passthrough: &'a dyn Fn() -> String,
}

pub(crate) fn run_pipeline(
    display_command: &str,
    cmd_for_behavior: CmdForBehavior<'_>,
    context: &ToolPermissionContext,
    tool_name: &str,
    predicate: RulePredicate,
    messages: PipelineMessages<'_>,
) -> PermissionResult {
    let deny_cmd = cmd_for_behavior(PermissionBehavior::Deny);
    if let Some(rule) = first_matching_rule(
        &context.always_deny_rules,
        &deny_cmd,
        PermissionBehavior::Deny,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Deny {
            message: (messages.deny)(display_command),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    let ask_cmd = cmd_for_behavior(PermissionBehavior::Ask);
    if let Some(rule) = first_matching_rule(
        &context.always_ask_rules,
        &ask_cmd,
        PermissionBehavior::Ask,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Ask {
            message: (messages.ask)(),
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    let allow_cmd = cmd_for_behavior(PermissionBehavior::Allow);
    if let Some(rule) = first_matching_rule(
        &context.always_allow_rules,
        &allow_cmd,
        PermissionBehavior::Allow,
        tool_name,
        predicate,
    ) {
        return PermissionResult::Allow {
            reason: PermissionDecisionReason::Rule { rule },
        };
    }

    let passthrough = (messages.passthrough)();
    PermissionResult::Passthrough {
        message: passthrough.clone(),
        reason: PermissionDecisionReason::Other {
            reason: passthrough,
        },
    }
}
