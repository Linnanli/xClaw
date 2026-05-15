//! `BashPermissionHook` — wraps the `dasclaw_bash_permissions` rule
//! engine (Slices 2.2.a-n) as a [`SafetyHook`] implementation.
//!
//! # Slice 2.2.f (hook adapter)
//!
//! Phase 2.2 of [Issue #490][issue-490]. Plan §5 row "2.2.f" defined
//! this slice as "`BashPermissionHook` impl + 接入 `CompositeSafetyHook`
//! 在 `BashValidationHook` 之后". PR #549 used the 2.2.f letter for the
//! `strip_env` wiring into `prefix_match`/`compound_match`; the hook
//! adapter — the actual integration point — remained TODO. This module
//! closes that gap. Tracker: issue #556.
//!
//! [issue-490]: https://github.com/Linnanli/xClaw/issues/490
//!
//! # Pipeline
//!
//! Only `before_tool_call` does real work; the other three [`SafetyHook`]
//! methods pass through so this hook composes cleanly with
//! `BashValidationHook` (Phase 2.1 command-injection gate) and other
//! safety hooks in a [`x_claw_agent::CompositeSafetyHook`] chain.
//!
//! For tools whose name matches [`BashPermissionHook::bash_tool_names`]
//! (default `["bash", "shell", "BashTool"]`), the hook:
//!
//! 1. Extracts the `command` field from `args`. Missing or non-string
//!    is Fail-Safe-mapped to [`SafetyDecision::Block`] (same shape as
//!    [`crate::BashValidationHook`]).
//! 2. Runs the rule pipeline:
//!    - [`check_exact_match`] first (upstream `bashPermissions.ts`
//!      L991, fast path), and if it returns
//!      [`PermissionResult::Passthrough`],
//!    - [`check_compound_match`] (compound splitting + per-subcommand
//!      prefix-match, upstream L1050).
//! 3. Maps the [`PermissionResult`] to a [`SafetyDecision`] per
//!    ADR-146:
//!    - `Allow` ⇒ [`SafetyDecision::Allow`].
//!    - `Deny { message, reason }` ⇒ [`SafetyDecision::Block { reason }`]
//!      with audit log `bash_perm::block`.
//!    - `Ask { message, reason }` ⇒ [`SafetyDecision::Ask`] with audit
//!      log `bash_perm::ask` and a [`RuleSuggestion`] list built from
//!      [`suggestion_for_exact_command`] (Slice 2.2.n generators).
//!    - `Passthrough { .. }` ⇒ [`SafetyDecision::Passthrough`] — this
//!      hook abstains and lets later hooks in the chain decide.
//!
//! Non-bash tools short-circuit to [`SafetyDecision::Allow`] so the
//! hook can be registered globally without disturbing other tools.
//!
//! # Audit log shape
//!
//! Decisions emit `tracing::warn!` with structured fields:
//!
//! | Field      | Source                                       |
//! |------------|----------------------------------------------|
//! | `tool`     | the tool name passed to `before_tool_call`   |
//! | `rule_id`  | Debug of the matched [`PermissionRule`] or `"Other"` |
//! | `message`  | the engine's `message` string                |
//!
//! Event message is `bash_perm::block` or `bash_perm::ask`. This
//! deliberately mirrors the `bash_security::block` shape from PR
//! #522/#524/#530 so SIEM filters can use the `bash_*::block` prefix.
//!
//! # What is NOT in this slice
//!
//! - `BashValidationHook` is still the security gate; this hook is
//!   added **after** it in the chain. Audit-log contract pinning (the
//!   2.2.h companion to PR #524's `bash_security::block` pin) is a
//!   follow-up PR.
//! - Desktop Ask UX is Phase 2.3.
//! - Configuration ingestion (loading the `ToolPermissionContext` from
//!   admin-backend / settings files) is Phase 2.3 too. For now callers
//!   construct the context directly and pass it in at hook
//!   construction time.

use async_trait::async_trait;
use dasclaw_bash_permissions::{
    BashRuleSuggestion, PermissionBehavior, PermissionDecisionReason, PermissionResult,
    PermissionRule, ToolPermissionContext, check_compound_match, check_exact_match,
    permission_rule_value_to_string, suggestion_for_exact_command,
};
use serde_json::Value;
use x_claw_agent::{RuleAction, RuleSuggestion, SafetyDecision, SafetyError, SafetyHook};

use crate::bash_validation_hook::DEFAULT_BASH_TOOL_NAMES;

/// `SafetyHook` adapter for the bash permission rule engine.
///
/// See module-level documentation for the contract.
#[derive(Debug, Clone)]
pub struct BashPermissionHook {
    context: ToolPermissionContext,
    bash_tool_names: Vec<String>,
}

impl BashPermissionHook {
    /// Create a hook with the given permission rule context.
    ///
    /// Uses [`crate::DEFAULT_BASH_TOOL_NAMES`] for the tool-name
    /// allowlist.
    #[must_use]
    pub fn new(context: ToolPermissionContext) -> Self {
        Self {
            context,
            bash_tool_names: DEFAULT_BASH_TOOL_NAMES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }

    /// Override the bash tool-name allowlist.
    #[must_use]
    pub fn with_tool_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bash_tool_names = names.into_iter().map(Into::into).collect();
        self
    }

    /// Returns `true` if `tool` should be treated as a bash invocation.
    fn is_bash_tool(&self, tool: &str) -> bool {
        self.bash_tool_names.iter().any(|n| n == tool)
    }
}

#[async_trait]
impl SafetyHook for BashPermissionHook {
    async fn before_prompt(&self, _prompt: &mut String) -> Result<SafetyDecision, SafetyError> {
        Ok(SafetyDecision::Allow)
    }

    async fn after_completion(&self, _completion: &mut String) -> Result<(), SafetyError> {
        Ok(())
    }

    async fn before_tool_call(
        &self,
        tool: &str,
        args: &mut Value,
    ) -> Result<SafetyDecision, SafetyError> {
        if !self.is_bash_tool(tool) {
            return Ok(SafetyDecision::Allow);
        }

        let command = match args.get("command").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return Ok(SafetyDecision::Block {
                    reason: "bash tool call missing required 'command' string field".to_string(),
                });
            }
        };

        // Upstream pipeline (bashPermissions.ts L1663-L1820): exact
        // match wins first (fast path, no AST cost). Only when the
        // engine has no exact opinion do we run the compound /
        // prefix / wildcard pipeline.
        let result = match check_exact_match(command, &self.context) {
            PermissionResult::Passthrough { .. } => check_compound_match(command, &self.context),
            decided => decided,
        };

        Ok(map_permission_result(tool, command, result))
    }

    async fn after_tool_output(
        &self,
        _tool: &str,
        _output: &mut String,
    ) -> Result<(), SafetyError> {
        Ok(())
    }
}

/// Maps a [`PermissionResult`] into a [`SafetyDecision`] and emits an
/// audit log line for `Deny` / `Ask` outcomes.
fn map_permission_result(tool: &str, command: &str, result: PermissionResult) -> SafetyDecision {
    match result {
        PermissionResult::Allow { .. } => SafetyDecision::Allow,
        PermissionResult::Deny { message, reason } => {
            let rule_id = format_rule_id(&reason);
            tracing::warn!(
                tool,
                rule_id = %rule_id,
                %message,
                "bash_perm::block"
            );
            SafetyDecision::Block {
                reason: format!("bash_perm::{rule_id}: {message}"),
            }
        }
        PermissionResult::Ask { message, reason } => {
            let rule_id = format_rule_id(&reason);
            tracing::warn!(
                tool,
                rule_id = %rule_id,
                %message,
                "bash_perm::ask"
            );
            SafetyDecision::Ask {
                reason: message,
                suggestions: build_rule_suggestions(command),
            }
        }
        PermissionResult::Passthrough { .. } => SafetyDecision::Passthrough,
    }
}

/// Renders a [`PermissionDecisionReason`] into the `rule_id` audit
/// surface. For `Rule` variants this is the upstream-equivalent
/// `"Bash(content)"` rendering of the matching rule's pattern. For
/// `Other` variants (cap exceeded, empty input, no rule fired) we use
/// the literal string `"Other"` so audit dashboards have a stable
/// dimension to filter on.
fn format_rule_id(reason: &PermissionDecisionReason) -> String {
    match reason {
        PermissionDecisionReason::Rule {
            rule: PermissionRule { rule_value, .. },
        } => permission_rule_value_to_string(
            &rule_value.tool_name,
            rule_value.rule_content.as_deref(),
        ),
        PermissionDecisionReason::Other { .. } => "Other".to_string(),
    }
}

/// Builds the [`RuleSuggestion`] list shown in the desktop Ask UX. Each
/// suggestion encodes the rule pattern (so the user can persist it
/// via a one-click "Always allow" button) plus a human-readable label.
fn build_rule_suggestions(command: &str) -> Vec<RuleSuggestion> {
    suggestion_for_exact_command(command)
        .into_iter()
        .map(rule_suggestion_from_bash)
        .collect()
}

fn rule_suggestion_from_bash(s: BashRuleSuggestion) -> RuleSuggestion {
    let rule_pattern = permission_rule_value_to_string(
        &s.rule_value.tool_name,
        s.rule_value.rule_content.as_deref(),
    );
    let action = match s.behavior {
        PermissionBehavior::Allow => RuleAction::Allow,
        PermissionBehavior::Deny => RuleAction::Deny,
        PermissionBehavior::Ask => RuleAction::Ask,
    };
    let label = match action {
        RuleAction::Allow => format!("Always allow `{rule_pattern}`"),
        RuleAction::Deny => format!("Always deny `{rule_pattern}`"),
        RuleAction::Ask => format!("Always ask for `{rule_pattern}`"),
        // `RuleAction` is `#[non_exhaustive]`; future variants fall back
        // to a generic label so the UX always has something to render.
        _ => format!("Rule `{rule_pattern}`"),
    };
    RuleSuggestion {
        label,
        rule_pattern,
        action,
    }
}
