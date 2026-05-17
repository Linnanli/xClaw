//! Phase 2.2 Slice 2.2.h — audit-log contract pinning for
//! [`BashPermissionHook`].
//!
//! Slice 2.2.f (PR #557, issue #556) introduced the hook adapter that
//! emits `tracing::warn!` events with the `bash_perm::block` /
//! `bash_perm::ask` event-message tags. The original wire-up tests in
//! `bash_permission_hook.rs` only proved the events *fire* (substring
//! `"bash_perm::block"` and `"rule_id="`). They did **not** pin:
//!
//! - the exact `rule_id` rendering for each rule shape (prefix /
//!   exact / wildcard / `Other`),
//! - the `message` field carrying the engine's human-facing copy,
//! - the `tool` field echoing the original tool name (so custom
//!   allowlist names are auditable),
//! - the absence of `bash_perm::*` events on Allow / Passthrough,
//! - the warn-level pinning (operators alert on warn, not info).
//!
//! This file (issue #559, Phase 2.2 Slice 2.2.h, companion to PR #524's
//! `bash_security::block` pin) closes those gaps. Naming follows the
//! `req_security_490_p2_2_h_audit_log_<scenario>` family so coverage
//! tooling groups them with the parallel `bash_security` audit-log pins
//! in [`bash_security_audit_log.rs`](./bash_security_audit_log.rs).
//!
//! ## Mutation-test protocol (Acceptance Criteria §3)
//!
//! Each pin in this file is paired with a deliberate-mutation thought
//! experiment recorded in PR #559's self-review checklist: rename or
//! drop the field on the hook side and at least one assertion here
//! must fail. The mutations live in PR description, not in code,
//! because committing dead `#[cfg(...)]` mutation harnesses would
//! itself be patch-style accretion.

use dasclaw_bash_permissions::{
    MAX_SUBCOMMANDS_FOR_SECURITY_CHECK, PermissionBehavior, PermissionRuleSource,
    ToolPermissionContext,
};
use dasclaw_hooks::BashPermissionHook;
use serde_json::json;
use tracing_test::traced_test;
use x_claw_agent::EgressDecision;

/// Project-scope rule context with a single rule. Mirrors
/// `bash_permission_hook.rs::ctx_with` but kept local — duplicating a
/// 6-line helper is cheaper than coupling two otherwise-unrelated test
/// files via a shared `mod common`.
fn ctx_with(behavior: PermissionBehavior, rule_content: &str) -> ToolPermissionContext {
    let mut ctx = ToolPermissionContext::default();
    ctx.add_rule(
        behavior,
        PermissionRuleSource::ProjectSettings,
        rule_content,
    );
    ctx
}

/// Fire the hook against `tool` with `command`, returning the decision.
/// Asserts the hook does not return [``] — every failure
/// mode of the hook is supposed to be encoded into the
/// [`EgressDecision`].
async fn fire_hook(hook: &BashPermissionHook, tool: &str, command: &str) -> EgressDecision {
    let args = json!({ "command": command });
    hook.validate_tool_call(tool, &args)
}

/// Convenience wrapper for the common single-rule-context case.
async fn fire(ctx: ToolPermissionContext, tool: &str, command: &str) -> EgressDecision {
    fire_hook(&BashPermissionHook::new(ctx), tool, command).await
}

// =====================================================================
// rule_id rendering — one pin per [`ShellPermissionRule`] shape
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_rule_id_prefix_syntax() {
    // Legacy `name:*` prefix syntax — the canonical rule shape for
    // "deny anything starting with `rm`". `rm -rf /tmp/x` is the
    // standard fixture (matches Slice 2.2.f hook_06).
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let decision = fire(ctx, "bash", "rm -rf /tmp/x").await;
    assert!(
        matches!(decision, EgressDecision::Block { .. }),
        "deny rule `rm:*` must Block; got {decision:?}"
    );

    assert!(
        logs_contain("bash_perm::block"),
        "audit log must carry `bash_perm::block` event-message tag"
    );
    // Exact substring — protects against accidental rendering drift
    // (e.g. `Bash(rm:*)` → `Bash[rm:*]` or `bash(rm:*)`).
    assert!(
        logs_contain("rule_id=Bash(rm:*)"),
        "prefix-syntax deny must surface `rule_id=Bash(rm:*)` literal"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_rule_id_exact_command() {
    // No `:*` / `*` / space-tail → engine parses as
    // `ShellPermissionRule::Exact { command: "git push" }`. Pin the
    // round-trip rendering for the exact-rule audit surface.
    let ctx = ctx_with(PermissionBehavior::Deny, "git push");
    let decision = fire(ctx, "bash", "git push").await;
    assert!(matches!(decision, EgressDecision::Block { .. }));

    assert!(logs_contain("bash_perm::block"));
    assert!(
        logs_contain("rule_id=Bash(git push)"),
        "exact-command deny must surface `rule_id=Bash(git push)` literal"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_rule_id_wildcard_pattern() {
    // Space + trailing `*` → `ShellPermissionRule::Wildcard`. Exact
    // mode rejects wildcards (per `exact_match.rs` comment), so the
    // match fires from `check_compound_match` → `check_prefix_match`.
    // Either way the surfaced `rule_value.rule_content` is the raw
    // pattern, so the audit rule_id is `Bash(git *)`.
    let ctx = ctx_with(PermissionBehavior::Deny, "git *");
    let decision = fire(ctx, "bash", "git status").await;
    assert!(
        matches!(decision, EgressDecision::Block { .. }),
        "wildcard `git *` must Block `git status`; got {decision:?}"
    );

    assert!(logs_contain("bash_perm::block"));
    assert!(
        logs_contain("rule_id=Bash(git *)"),
        "wildcard deny must surface `rule_id=Bash(git *)` literal"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_rule_id_other_for_cap_exceeded() {
    // Construct a compound with > MAX_SUBCOMMANDS_FOR_SECURITY_CHECK
    // subcommands. `check_compound_match` returns
    // `PermissionResult::Ask` with `PermissionDecisionReason::Other`,
    // which the hook renders as the literal `rule_id=Other` audit
    // surface (no `Bash(...)` wrapper because no rule matched).
    let parts: Vec<String> = (0..=MAX_SUBCOMMANDS_FOR_SECURITY_CHECK)
        .map(|i| format!("echo {i}"))
        .collect();
    let command = parts.join(" && ");

    let decision = fire(ToolPermissionContext::default(), "bash", &command).await;
    assert!(
        matches!(decision, EgressDecision::Ask { .. }),
        "over-cap compound must Ask (upstream-parity, never auto-Deny); got {decision:?}"
    );

    assert!(
        logs_contain("bash_perm::ask"),
        "cap-exceeded Ask must emit the `bash_perm::ask` audit tag"
    );
    assert!(
        logs_contain("rule_id=Other"),
        "non-rule decision reasons must surface `rule_id=Other` literal"
    );
    // Negative pin — cap-exceeded must NOT impersonate a real rule.
    assert!(
        !logs_contain("rule_id=Bash("),
        "cap-exceeded must not surface a synthetic `Bash(...)` rule_id"
    );
}

// =====================================================================
// `message` field — engine copy is the audit surface, not the
// EgressDecision wrapper.
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_message_field_block_carries_engine_copy() {
    // The hook passes the engine's message via `%message` so it lands
    // in the audit-line body slot rather than as a `message=`
    // structured field. Pin the *full* engine copy verbatim so any
    // wording drift ("denied" → "blocked", tool-name substitution,
    // command echo dropped, …) trips this test. Operators grep on
    // both "denied" and the surrounding tool/command context.
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let decision = fire(ctx, "bash", "rm -rf /tmp/x").await;
    assert!(matches!(decision, EgressDecision::Block { .. }));

    assert!(logs_contain("bash_perm::block"));
    assert!(
        logs_contain("Permission to use Bash with command rm -rf /tmp/x has been denied."),
        "block audit must carry the full engine deny copy verbatim"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_message_field_ask_carries_engine_copy() {
    // Same idea as the block pin: pin the full ask copy verbatim.
    // Use a prefix rule + matching command to fire the single-
    // subcommand ask path (cap-exceeded shares the same string but is
    // covered by the `rule_id_other_for_cap_exceeded` pin above).
    let ctx = ctx_with(PermissionBehavior::Ask, "git push:*");
    let decision = fire(ctx, "bash", "git push").await;
    assert!(matches!(decision, EgressDecision::Ask { .. }));

    assert!(logs_contain("bash_perm::ask"));
    assert!(
        logs_contain("Claude requested permissions to use Bash, but you haven't granted it yet."),
        "ask audit must carry the full engine ask copy verbatim"
    );
}

// =====================================================================
// `tool` field — original tool name, not a hard-coded constant
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_tool_field_default_bash() {
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let decision = fire(ctx, "bash", "rm -rf /tmp/x").await;
    assert!(matches!(decision, EgressDecision::Block { .. }));

    // `tracing` renders `tool: &str` as `tool="bash"` under
    // `FmtSubscriber` (see tracing-test 0.2 subscriber config). Allow
    // both quoted and bare forms for forward compat.
    assert!(
        logs_contain("tool=\"bash\"") || logs_contain("tool=bash"),
        "audit log must echo the originating tool name `bash`"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_tool_field_custom_allowlist_name() {
    // Custom allowlist — the hook's `tool` audit field must reflect
    // the parameter passed to `before_tool_call`, not the constant
    // `BASH_TOOL_NAME = "Bash"` used internally by the engine for the
    // `rule_id` `Bash(...)` wrapper. This is the canonical scenario
    // where a host registers their own shell tool (e.g. VS Code's
    // `RunInTerminal`) under the bash rule engine.
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let hook = BashPermissionHook::new(ctx).with_tool_names(["RunInTerminal"]);
    let decision = fire_hook(&hook, "RunInTerminal", "rm -rf /tmp/x").await;
    assert!(matches!(decision, EgressDecision::Block { .. }));

    assert!(logs_contain("bash_perm::block"));
    assert!(
        logs_contain("tool=\"RunInTerminal\"") || logs_contain("tool=RunInTerminal"),
        "audit log must echo the custom tool name, not the engine's `Bash` constant"
    );
    // `rule_id` is keyed on the rule's `tool_name` (engine internal),
    // which is still `Bash`. This split between `tool=` (caller's
    // name) and `rule_id=Bash(...)` (rule's tool_name) is intentional
    // — pin both directions so a refactor that conflates them fails
    // here.
    assert!(
        logs_contain("rule_id=Bash(rm:*)"),
        "rule_id wrapper must stay anchored to the engine's `Bash` tool_name"
    );
}

// =====================================================================
// Silence pins — Allow / Passthrough emit zero `bash_perm::*` events
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_silent_on_empty_command_fail_safe() {
    // Issue #559 lists "rule_id = `Other` for empty command" as a
    // defensive pin. Trace through the hook: empty / non-string
    // command short-circuits to `EgressDecision::Block` with the
    // `bash tool call missing required 'command' string field`
    // wrapper **before** any engine call. So the right contract is:
    // Fail-Safe Block fires zero `bash_perm::*` audit events (the
    // audit surface is the engine's decision channel, not the input-
    // validation channel). Pin this so a future refactor that moves
    // empty-command handling into the engine doesn't silently change
    // the audit surface (operators would suddenly see ask/block
    // events for malformed args, polluting SIEM dashboards).
    let decision = fire(ToolPermissionContext::default(), "bash", "").await;
    assert!(
        matches!(decision, EgressDecision::Block { .. }),
        "empty command must Fail-Safe Block; got {decision:?}"
    );
    assert!(
        !logs_contain("bash_perm::block"),
        "empty-command Fail-Safe must not emit a `bash_perm::block` engine audit event"
    );
    assert!(
        !logs_contain("bash_perm::ask"),
        "empty-command Fail-Safe must not emit a `bash_perm::ask` engine audit event"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_silent_on_allow() {
    let ctx = ctx_with(PermissionBehavior::Allow, "git status");
    let decision = fire(ctx, "bash", "git status").await;
    assert_eq!(decision, EgressDecision::Allow);

    // Mirror `req_security_490_p2_1_d_audit_log_silent_on_safe_command`
    // — Allow path must emit zero `bash_perm::*` events so SIEM rules
    // can alert on the prefix without false positives.
    assert!(
        !logs_contain("bash_perm::block"),
        "Allow path must not emit `bash_perm::block`"
    );
    assert!(
        !logs_contain("bash_perm::ask"),
        "Allow path must not emit `bash_perm::ask`"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_silent_on_passthrough() {
    // Empty context → engine returns Passthrough → hook returns
    // `EgressDecision::Passthrough`. No audit event should fire
    // because the hook abstained.
    let decision = fire(ToolPermissionContext::default(), "bash", "git status").await;
    assert_eq!(decision, EgressDecision::Passthrough);

    assert!(!logs_contain("bash_perm::block"));
    assert!(!logs_contain("bash_perm::ask"));
}

// =====================================================================
// Level pin — warn, not info / error
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_2_h_audit_log_event_level_is_warn() {
    // `tracing-test` configures `FmtSubscriber` with `with_level(true)
    // .with_ansi(false)` (see tracing-test/src/subscriber.rs), so each
    // event line carries an unstyled level tag like ` WARN ` between
    // the timestamp and the event body. Pinning ` WARN ` + absence of
    // ` INFO ` / ` ERROR ` ensures a future hook refactor cannot
    // silently demote the audit signal to a level that SIEM
    // rules-of-thumb would ignore (info) or escalate inappropriately
    // (error implies an unhandled engine failure).
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let decision = fire(ctx, "bash", "rm -rf /tmp/x").await;
    assert!(matches!(decision, EgressDecision::Block { .. }));

    assert!(logs_contain("bash_perm::block"));
    assert!(
        logs_contain(" WARN "),
        "audit event must be emitted at WARN level"
    );
    assert!(
        !logs_contain(" INFO "),
        "audit event must not be demoted to INFO (SIEM filters target warn)"
    );
    assert!(
        !logs_contain(" ERROR "),
        "audit event must not be promoted to ERROR (reserved for engine failures)"
    );
}
