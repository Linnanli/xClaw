//! Context → `PermissionRule` collectors — Slice 2.2.k (Issue #490 Phase 2.2).
//!
//! Semantic port of upstream `permissions.ts` `getAllowRules` (L122-L130),
//! `getDenyRules` (L213-L221), `getAskRules` (L223-L231).
//!
//! ## Storage shape note
//!
//! Upstream stores raw `"Bash(content)"` strings and parses each lookup via
//! `permissionRuleValueFromString` (Slice 2.2.j). Our
//! [`ToolPermissionContext`] stores **inner content only** (`"npm install"`,
//! `"git:*"`, `""`) because the crate is bash-only and the `"Bash(...)"`
//! wrapper is implicit. The collectors therefore skip the parser and
//! assemble [`PermissionRuleValue`]s directly with `tool_name = "Bash"`.
//!
//! The Slice 2.2.j parser is **still the right bridge** for ingesting
//! settings JSON in upstream format (e.g. a `claw-code` ↔ upstream cross-
//! compat loader); that ingestion path is Slice 2.2.l.
//!
//! ## Parity with upstream
//!
//! Upstream `permissionRuleValueFromString` collapses `""` and `"*"` to a
//! tool-wide rule (`rule_content: None`) — see Slice 2.2.j tests 13. We
//! reproduce that collapse here so that the same `(behavior, source)`
//! combinations yield the same `PermissionRule` shape regardless of which
//! storage layer fed the context.
//!
//! ## Determinism
//!
//! [`ToolPermissionContext`] uses `BTreeMap<PermissionRuleSource, Vec<String>>`
//! precisely so iteration is sorted by source — critical for reproducible
//! [`crate::shadowed_rule_detection::detect_unreachable_rules`] output
//! (which uses `find()`, first-match-wins).

use crate::exact_match::BASH_TOOL_NAME;
use crate::types::{
    PermissionBehavior, PermissionRule, PermissionRuleValue, ToolPermissionContext,
    ToolPermissionRulesBySource,
};

/// Collect all `Allow` rules from `ctx` as fully-shaped [`PermissionRule`]s.
///
/// Upstream `getAllowRules` L122-L130.
pub fn get_allow_rules(ctx: &ToolPermissionContext) -> Vec<PermissionRule> {
    collect(&ctx.always_allow_rules, PermissionBehavior::Allow)
}

/// Collect all `Deny` rules from `ctx`.
///
/// Upstream `getDenyRules` L213-L221.
pub fn get_deny_rules(ctx: &ToolPermissionContext) -> Vec<PermissionRule> {
    collect(&ctx.always_deny_rules, PermissionBehavior::Deny)
}

/// Collect all `Ask` rules from `ctx`.
///
/// Upstream `getAskRules` L223-L231.
pub fn get_ask_rules(ctx: &ToolPermissionContext) -> Vec<PermissionRule> {
    collect(&ctx.always_ask_rules, PermissionBehavior::Ask)
}

/// Common pipeline shared by the three public collectors. Avoids
/// patch-style duplication — single code path, behavior parameterised.
fn collect(
    by_source: &ToolPermissionRulesBySource,
    behavior: PermissionBehavior,
) -> Vec<PermissionRule> {
    let mut out = Vec::with_capacity(by_source.values().map(Vec::len).sum());
    for (source, contents) in by_source {
        for content in contents {
            // Collapse upstream's "tool-wide" forms (`""` and `"*"`) to
            // `rule_content: None` — parity with 2.2.j parser L132-L134.
            let rule_content = if content.is_empty() || content == "*" {
                None
            } else {
                Some(content.clone())
            };
            out.push(PermissionRule {
                source: *source,
                rule_behavior: behavior,
                rule_value: PermissionRuleValue {
                    tool_name: BASH_TOOL_NAME.to_string(),
                    rule_content,
                },
            });
        }
    }
    out
}
