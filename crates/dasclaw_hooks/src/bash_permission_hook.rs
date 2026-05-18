//! `BashPermissionHook` — wraps the `dasclaw_bash_permissions` rule
//! engine (Slices 2.2.a-n) as a [`EgressGate`] implementation.
//!
//! # Slice 2.2.f (hook adapter)
//!
//! Phase 2.2 of [Issue #490][issue-490]. Plan §5 row "2.2.f" defined
//! this slice as "`BashPermissionHook` impl + 接入 `CompositeEgressGate`
//! 在 `BashValidationHook` 之后". PR #549 used the 2.2.f letter for the
//! `strip_env` wiring into `prefix_match`/`compound_match`; the hook
//! adapter — the actual integration point — remained TODO. This module
//! closes that gap. Tracker: issue #556.
//!
//! [issue-490]: https://github.com/Linnanli/xClaw/issues/490
//!
//! # Pipeline
//!
//! Only the `EgressKind::ToolExecution` branch of `check` does real work;
//! every other kind returns [`EgressDecision::Allow`] so this gate
//! composes cleanly with `BashValidationHook` (Phase 2.1 command-injection
//! gate) and other egress gates in a [`dasclaw_core::CompositeEgressGate`]
//! chain.
//!
//! For tools whose name matches [`BashPermissionHook::bash_tool_names`]
//! (default `["bash", "shell", "BashTool"]`), the hook:
//!
//! 1. Extracts the `command` field from `args`. Missing or non-string
//!    is Fail-Safe-mapped to [`EgressDecision::Block`] (same shape as
//!    [`crate::BashValidationHook`]).
//! 2. Runs the rule pipeline:
//!    - [`check_exact_match`] first (upstream `bashPermissions.ts`
//!      L991, fast path), and if it returns
//!      [`PermissionResult::Passthrough`],
//!    - [`check_compound_match`] (compound splitting + per-subcommand
//!      prefix-match, upstream L1050).
//! 3. Maps the [`PermissionResult`] to a [`EgressDecision`] per
//!    ADR-146:
//!    - `Allow` ⇒ [`EgressDecision::Allow`].
//!    - `Deny { message, reason }` ⇒ [`EgressDecision::Block { reason }`]
//!      with audit log `bash_perm::block`.
//!    - `Ask { message, reason }` ⇒ [`EgressDecision::Ask`] with audit
//!      log `bash_perm::ask` and a [`RuleSuggestion`] list built from
//!      [`suggestion_for_exact_command`] (Slice 2.2.n generators).
//!    - `Passthrough { .. }` ⇒ [`EgressDecision::Passthrough`] — this
//!      hook abstains and lets later hooks in the chain decide.
//!
//! Non-bash tools short-circuit to [`EgressDecision::Allow`] so the
//! hook can be registered globally without disturbing other tools.
//!
//! # Audit log shape
//!
//! Decisions emit `tracing::warn!` with structured fields:
//!
//! | Field      | Source                                       |
//! |------------|----------------------------------------------|
//! | `tool`     | the tool name passed to `EgressGate::check`  |
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
//!
//! # Phase 4.2 strict sed allowlist (opt-in)
//!
//! Issue #607 / Phase 4.2 of [Issue #490][issue-490]. After PR #606 landed
//! the upstream-parity strict sed allowlist function
//! [`dasclaw_bash_validation::sed_command_is_allowed_by_allowlist`], the
//! hook can optionally use it to **upgrade** a rule-pipeline
//! `Passthrough` or `Ask` outcome to `Allow` when the command matches
//! the upstream Pattern 1 (line print with `-n`) or Pattern 2 (single
//! substitution) constraints. Enable via
//! [`BashPermissionHook::with_sed_strict_allowlist`]; default off.
//!
//! Invariants (pinned by `tests/bash_permission_sed_strict_allowlist.rs`):
//!
//! - **Never downgrades**: `Block` / `Allow` are passed through
//!   untouched. Deny remains sovereign (PR #557 precedence).
//! - **Only upgrades** `Passthrough` and `Ask` decisions, so the strict
//!   mode cannot accidentally short-circuit a hook earlier in the chain
//!   that wanted to Block.
//! - **Non-sed → no-op**: the allowlist function returns `false` for
//!   anything that is not a single `sed` simple command, so non-sed
//!   inputs fall through unchanged.
//! - Upgrades emit a `tracing::info!` event `bash_perm::allow_sed_strict_upgrade`
//!   with `tool` + `rule_id="SedAllowlist"` for SIEM dashboards.

use async_trait::async_trait;
use dasclaw_bash_permissions::{
    BashRuleSuggestion, PermissionBehavior, PermissionDecisionReason, PermissionResult,
    PermissionRule, ToolPermissionContext, check_compound_match, check_exact_match,
    permission_rule_value_to_string, suggestion_for_exact_command,
};
use dasclaw_bash_validation::{
    command_has_any_git, command_writes_to_git_internal_paths, is_unsafe_xargs_invocation,
    sed_command_is_allowed_by_allowlist,
};
use dasclaw_core::{
    EgressDecision, EgressGate, EgressKind, RedactionStats, RuleAction, RuleSuggestion,
};
use serde_json::Value;

use crate::bash_validation_hook::DEFAULT_BASH_TOOL_NAMES;

/// `EgressGate` adapter for the bash permission rule engine.
///
/// See module-level documentation for the contract.
#[derive(Debug, Clone)]
pub struct BashPermissionHook {
    context: ToolPermissionContext,
    bash_tool_names: Vec<String>,
    /// `Some` enables the Phase 4.2 strict sed allowlist upgrade. The
    /// inner `bool` mirrors upstream `acceptEdits` → when `true`,
    /// `sed -i` / file-write substitutions are also pre-approved.
    /// `None` (default) keeps the pre-4.2 behavior.
    sed_strict_allowlist: Option<SedStrictMode>,
}

/// Strict sed allowlist configuration for [`BashPermissionHook`]. See
/// the Phase 4.2 section of the module-level documentation for the
/// contract.
#[derive(Debug, Clone, Copy)]
struct SedStrictMode {
    allow_file_writes: bool,
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
            sed_strict_allowlist: None,
        }
    }

    /// Enable the Phase 4.2 strict sed allowlist upgrade.
    ///
    /// When enabled, after the rule pipeline returns `Passthrough` or
    /// `Ask`, the hook checks whether the command matches the strict
    /// upstream sed allowlist (Pattern 1 line print with `-n` or
    /// Pattern 2 single substitution); if so, the decision is upgraded
    /// to `Allow`. `Block` and `Allow` are never modified.
    ///
    /// `allow_file_writes` mirrors upstream `acceptEdits` mode: when
    /// `true`, `sed -i ...` and trailing file arguments are tolerated.
    /// Set to `false` in read-only contexts.
    #[must_use]
    pub fn with_sed_strict_allowlist(mut self, allow_file_writes: bool) -> Self {
        self.sed_strict_allowlist = Some(SedStrictMode { allow_file_writes });
        self
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

    /// Phase 4.2 strict sed allowlist upgrade.
    ///
    /// Returns `decision` unchanged unless **all** of the following hold:
    ///
    /// 1. Strict mode was enabled via
    ///    [`Self::with_sed_strict_allowlist`].
    /// 2. `decision` is `Passthrough` or `Ask` (the only two
    ///    rule-pipeline outcomes the issue contract allows upgrading).
    /// 3. [`sed_command_is_allowed_by_allowlist`] returns `true` for
    ///    the command, i.e. it is a single `sed` invocation matching
    ///    upstream Pattern 1 or Pattern 2 and denylist-clean.
    ///
    /// On upgrade, emits `tracing::info!("bash_perm::allow_sed_strict_upgrade", ...)`
    /// with `tool` + `rule_id="SedAllowlist"` so SIEM can pin a stable
    /// channel separate from `bash_perm::block` / `bash_perm::ask`.
    fn maybe_upgrade_sed_strict_allowlist(
        &self,
        tool: &str,
        command: &str,
        decision: EgressDecision,
    ) -> EgressDecision {
        let Some(mode) = self.sed_strict_allowlist else {
            return decision;
        };

        // Only Passthrough / Ask are upgrade candidates. Block / Allow
        // pass through untouched — strict mode never downgrades a Deny
        // and never re-logs an existing Allow.
        if !matches!(
            decision,
            EgressDecision::Passthrough | EgressDecision::Ask { .. }
        ) {
            return decision;
        }

        if !sed_command_is_allowed_by_allowlist(command, mode.allow_file_writes) {
            return decision;
        }

        tracing::info!(
            tool,
            rule_id = "SedAllowlist",
            allow_file_writes = mode.allow_file_writes,
            "bash_perm::allow_sed_strict_upgrade"
        );
        EgressDecision::Allow
    }
}

impl BashPermissionHook {
    /// Core decision logic — exposed so unit and integration tests can drive
    /// the gate without serialising args through JSON. Returns
    /// [`EgressDecision::Allow`] for non-bash tools.
    pub fn validate_tool_call(&self, tool: &str, args: &Value) -> EgressDecision {
        if !self.is_bash_tool(tool) {
            return EgressDecision::Allow;
        }

        let command = match args.get("command").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => s,
            _ => {
                return EgressDecision::Block {
                    reason: "bash tool call missing required 'command' string field".to_string(),
                    stats: RedactionStats::default(),
                };
            }
        };

        // Phase 3.2.E.rest (issue #603): hard-deny sandbox-escape pins
        // **S21** (`xargs` targeting a non-safe command) and **S22**
        // (write into `HEAD` / `objects/` / `refs/` / `hooks/` when
        // the compound also invokes git) BEFORE the rule pipeline.
        // No user permission rule may legitimately allow these — they
        // are validator-defeating attacks that bypass the readonly
        // allowlist or the .git/-relative bare-repo protection.
        if let Some(reason) = detect_sandbox_escape(command) {
            return map_permission_result(
                tool,
                command,
                PermissionResult::Deny {
                    message: sandbox_escape_message(&reason),
                    reason,
                },
            );
        }

        // Upstream pipeline (bashPermissions.ts L1663-L1820): exact
        // match wins first (fast path, no AST cost). Only when the
        // engine has no exact opinion do we run the compound /
        // prefix / wildcard pipeline.
        let result = match check_exact_match(command, &self.context) {
            PermissionResult::Passthrough { .. } => check_compound_match(command, &self.context),
            decided => decided,
        };

        let decision = map_permission_result(tool, command, result);
        self.maybe_upgrade_sed_strict_allowlist(tool, command, decision)
    }
}

#[async_trait]
impl EgressGate for BashPermissionHook {
    async fn check(&self, kind: &EgressKind, payload: &str) -> EgressDecision {
        // ADR-148: only the tool-execution egress kind carries bash
        // command arguments; every other kind is a no-op for this gate.
        let tool = match kind {
            EgressKind::ToolExecution { tool } => tool,
            _ => return EgressDecision::Allow,
        };
        // Fail-Safe: a payload that does not deserialise into a JSON
        // object is treated as a structurally invalid bash call.
        let args: Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                return EgressDecision::Block {
                    reason: format!("bash permission gate: malformed tool args payload: {e}"),
                    stats: RedactionStats::default(),
                };
            }
        };
        self.validate_tool_call(tool, &args)
    }
}

/// Maps a [`PermissionResult`] into a [`EgressDecision`] and emits an
/// audit log line for `Deny` / `Ask` outcomes.
fn map_permission_result(tool: &str, command: &str, result: PermissionResult) -> EgressDecision {
    match result {
        PermissionResult::Allow { .. } => EgressDecision::Allow,
        PermissionResult::Deny { message, reason } => {
            let rule_id = format_rule_id(&reason);
            tracing::warn!(
                tool,
                rule_id = %rule_id,
                %message,
                "bash_perm::block"
            );
            EgressDecision::Block {
                reason: format!("bash_perm::{rule_id}: {message}"),
                stats: RedactionStats::default(),
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
            EgressDecision::Ask {
                reason: message,
                suggestions: build_rule_suggestions(command),
            }
        }
        PermissionResult::Passthrough { .. } => EgressDecision::Passthrough,
    }
}

/// Renders a [`PermissionDecisionReason`] into the `rule_id` audit
/// surface. For `Rule` variants this is the upstream-equivalent
/// `"Bash(content)"` rendering of the matching rule's pattern. For
/// `Other` variants (cap exceeded, empty input, no rule fired) we use
/// the literal string `"Other"` so audit dashboards have a stable
/// dimension to filter on. The two sandbox-escape variants
/// ([`PermissionDecisionReason::FlagNotInAllowlist`] /
/// [`PermissionDecisionReason::GitInternalPathWrite`], added by Phase
/// 3.2.E.rest) render as their bare variant name so the
/// `bash_perm::<rule_id>` audit channel can filter on them
/// alongside rule-driven blocks.
fn format_rule_id(reason: &PermissionDecisionReason) -> String {
    match reason {
        PermissionDecisionReason::Rule {
            rule: PermissionRule { rule_value, .. },
        } => permission_rule_value_to_string(
            &rule_value.tool_name,
            rule_value.rule_content.as_deref(),
        ),
        PermissionDecisionReason::Other { .. } => "Other".to_string(),
        PermissionDecisionReason::FlagNotInAllowlist { .. } => "FlagNotInAllowlist".to_string(),
        PermissionDecisionReason::GitInternalPathWrite { .. } => "GitInternalPathWrite".to_string(),
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

/// Sandbox-escape pre-check — runs before the rule pipeline so the
/// permission engine cannot be coerced into allowing pins **S21** /
/// **S22** via a user-installed `allow` rule.
///
/// Returns `Some(reason)` when the command matches one of the two
/// validator-defeating patterns documented on
/// [`PermissionDecisionReason::FlagNotInAllowlist`] /
/// [`PermissionDecisionReason::GitInternalPathWrite`]:
///
/// * **S21** — `xargs` invocation whose target is not in
///   `SAFE_TARGET_COMMANDS_FOR_XARGS` (delegated to
///   [`is_unsafe_xargs_invocation`]).
/// * **S22** — a compound that both contains a git invocation and
///   writes into one of the four git-internal roots
///   (`HEAD` / `objects/` / `refs/` / `hooks/`). Both halves of the
///   conjunction are required (upstream parity, `readOnlyValidation.ts`
///   L1838-L1842): a bare `echo foo > hooks/x` with no git in the
///   compound is not the bare-repo masquerade attack and should fall
///   through to the rule engine.
///
/// Precedence: S22 is checked before S21 so the audit log surfaces
/// the more-specific git-internal reason when both fire (e.g.,
/// `xargs git commit && echo m > hooks/x`).
fn detect_sandbox_escape(command: &str) -> Option<PermissionDecisionReason> {
    if command_has_any_git(command) && command_writes_to_git_internal_paths(command) {
        return Some(PermissionDecisionReason::GitInternalPathWrite {
            command: command.to_string(),
        });
    }
    if is_unsafe_xargs_invocation(command) {
        return Some(PermissionDecisionReason::FlagNotInAllowlist {
            command: command.to_string(),
        });
    }
    None
}

/// Renders the user-visible block message for a sandbox-escape reason.
/// The message text is the audit-log `message` field — kept stable so
/// SIEM dashboards / snapshot tests can pin on it.
fn sandbox_escape_message(reason: &PermissionDecisionReason) -> String {
    match reason {
        PermissionDecisionReason::FlagNotInAllowlist { command } => format!(
            "bash sandbox escape S21: `xargs` target / flag is not in the safe allowlist \
             (command: {command})"
        ),
        PermissionDecisionReason::GitInternalPathWrite { command } => format!(
            "bash sandbox escape S22: command writes into a git-internal path \
             (HEAD / objects/ / refs/ / hooks/) inside a git-bearing compound \
             (command: {command})"
        ),
        // The other variants are never returned by `detect_sandbox_escape`;
        // fall back to a generic surface so the function is total.
        PermissionDecisionReason::Rule { .. } | PermissionDecisionReason::Other { .. } => {
            "bash sandbox escape".to_string()
        }
    }
}
