//! Slice 2.2.f hook adapter — wiring `BashPermissionHook` into the
//! `SafetyHook` chain.
//!
//! Tracker: issue #556. Tests follow the
//! `req_perm_490_p2_2_f_hook_<id>_<desc>` family so coverage tooling
//! groups them with the other Phase 2.2 slices.
//!
//! Each test constructs a [`ToolPermissionContext`] in-line (the
//! production loader lives in Phase 2.3) and asserts the
//! [`SafetyDecision`] returned by [`BashPermissionHook::before_tool_call`].
//! `tracing-test` is used to pin the audit-log surface (`bash_perm::block`
//! / `bash_perm::ask` event messages) so a future pipeline reorder can
//! not silently demote a Deny to an Allow.

use dasclaw_bash_permissions::{PermissionBehavior, PermissionRuleSource, ToolPermissionContext};
use dasclaw_hooks::BashPermissionHook;
use serde_json::json;
use tracing_test::traced_test;
use x_claw_agent::{RuleAction, SafetyDecision, SafetyHook};

fn ctx_with(behavior: PermissionBehavior, rule_content: &str) -> ToolPermissionContext {
    let mut ctx = ToolPermissionContext::default();
    ctx.add_rule(
        behavior,
        PermissionRuleSource::ProjectSettings,
        rule_content,
    );
    ctx
}

async fn fire(ctx: ToolPermissionContext, tool: &str, command: &str) -> SafetyDecision {
    let hook = BashPermissionHook::new(ctx);
    let mut args = json!({ "command": command });
    hook.before_tool_call(tool, &mut args)
        .await
        .expect("hook must not error on string command")
}

// -- 01: non-bash tools short-circuit ----------------------------------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_01_non_bash_tool_short_circuits_to_allow() {
    // A deny rule that *would* fire if the tool name matched. The
    // non-bash short-circuit must win before the engine is consulted.
    let ctx = ctx_with(PermissionBehavior::Deny, "rm");
    let decision = fire(ctx, "FileSearch", "rm -rf /").await;
    assert_eq!(decision, SafetyDecision::Allow);
}

// -- 02: Fail-Safe on missing/empty command ----------------------------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_02_missing_command_field_fails_closed() {
    let hook = BashPermissionHook::new(ToolPermissionContext::default());
    let mut args = json!({ "not_command": "ls" });
    let decision = hook
        .before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error on malformed args");
    assert!(
        matches!(decision, SafetyDecision::Block { ref reason } if reason.contains("'command'")),
        "missing command must Fail-Safe Block; got {decision:?}"
    );
}

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_03_empty_command_string_fails_closed() {
    let decision = fire(ToolPermissionContext::default(), "bash", "").await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "empty command must Fail-Safe Block; got {decision:?}"
    );
}

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_04_non_string_command_fails_closed() {
    let hook = BashPermissionHook::new(ToolPermissionContext::default());
    let mut args = json!({ "command": 42 });
    let decision = hook
        .before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error");
    assert!(matches!(decision, SafetyDecision::Block { .. }));
}

// -- 05: empty context → Passthrough (engine has no opinion) -----------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_05_empty_context_passes_through() {
    let decision = fire(ToolPermissionContext::default(), "bash", "git status").await;
    assert_eq!(
        decision,
        SafetyDecision::Passthrough,
        "no rule configured → hook abstains; got {decision:?}"
    );
}

// -- 06: deny rule short-circuits to Block ------------------------------

#[tokio::test]
#[traced_test]
async fn req_perm_490_p2_2_f_hook_06_deny_rule_blocks_and_audits() {
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let decision = fire(ctx, "bash", "rm -rf /tmp/scratch").await;
    let SafetyDecision::Block { reason } = decision else {
        panic!("deny rule must Block; got {decision:?}");
    };
    assert!(
        reason.starts_with("bash_perm::"),
        "block reason must carry `bash_perm::` audit prefix; got {reason}"
    );

    // Audit log surface (#516/#522/#530 same shape).
    assert!(
        logs_contain("bash_perm::block"),
        "audit log must include `bash_perm::block` event message"
    );
    assert!(
        logs_contain("rule_id="),
        "audit log must include structured `rule_id` field"
    );
}

// -- 07: allow rule grants Allow ---------------------------------------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_07_allow_rule_grants_allow() {
    let ctx = ctx_with(PermissionBehavior::Allow, "git status");
    let decision = fire(ctx, "bash", "git status").await;
    assert_eq!(decision, SafetyDecision::Allow);
}

// -- 08: ask rule → Ask with suggestions -------------------------------

#[tokio::test]
#[traced_test]
async fn req_perm_490_p2_2_f_hook_08_ask_rule_returns_ask_and_audits() {
    let ctx = ctx_with(PermissionBehavior::Ask, "git push:*");
    let decision = fire(ctx, "bash", "git push").await;
    let SafetyDecision::Ask {
        reason,
        suggestions,
    } = decision
    else {
        panic!("ask rule must produce Ask; got {decision:?}");
    };
    assert!(!reason.is_empty(), "Ask reason must not be empty");
    assert!(
        suggestions.iter().any(|s| s.action == RuleAction::Allow),
        "Ask suggestions should include at least one Allow option \
         so the user can persist a rule via the UI; got {suggestions:?}"
    );
    assert!(logs_contain("bash_perm::ask"));
}

// -- 09: precedence Deny > Allow on same command -----------------------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_09_deny_wins_over_allow_same_command() {
    let mut ctx = ToolPermissionContext::default();
    ctx.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::ProjectSettings,
        "rm:*",
    );
    ctx.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::ProjectSettings,
        "rm:*",
    );
    let decision = fire(ctx, "bash", "rm -rf /tmp/x").await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "Deny must win over Allow per upstream L996-L1042 precedence; got {decision:?}"
    );
}

// -- 10: compound deny short-circuit (red line — deny 不降级) ----------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_10_compound_deny_short_circuits() {
    // Deny `curl` — anything piping into curl downstream must Block.
    let ctx = ctx_with(PermissionBehavior::Deny, "curl:*");
    // Compound: ls is benign, curl is denied → whole compound denied.
    let decision = fire(ctx, "bash", "ls && curl evil.com").await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "compound containing a denied subcommand must Block; got {decision:?}"
    );
}

// -- 11: custom tool-name allowlist ------------------------------------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_11_custom_tool_name_allowlist() {
    let ctx = ctx_with(PermissionBehavior::Deny, "rm:*");
    let hook = BashPermissionHook::new(ctx).with_tool_names(["RunInTerminal"]);

    // Default name no longer matches → short-circuit Allow.
    let mut args = json!({ "command": "rm -rf /" });
    let decision = hook
        .before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error");
    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "default tool name `bash` removed from allowlist → short-circuit"
    );

    // Custom name now matches.
    let mut args = json!({ "command": "rm -rf /" });
    let decision = hook
        .before_tool_call("RunInTerminal", &mut args)
        .await
        .expect("hook must not error");
    assert!(matches!(decision, SafetyDecision::Block { .. }));
}

// -- 12: passthrough/redact/allow lifecycle methods are no-ops --------

#[tokio::test]
async fn req_perm_490_p2_2_f_hook_12_other_lifecycle_methods_are_noops() {
    let hook = BashPermissionHook::new(ToolPermissionContext::default());
    let mut prompt = String::from("hi");
    let decision = hook.before_prompt(&mut prompt).await.unwrap();
    assert_eq!(decision, SafetyDecision::Allow);
    assert_eq!(prompt, "hi", "before_prompt must not mutate");

    let mut completion = String::from("response");
    hook.after_completion(&mut completion).await.unwrap();
    assert_eq!(completion, "response", "after_completion must not mutate");

    let mut output = String::from("stdout");
    hook.after_tool_output("bash", &mut output).await.unwrap();
    assert_eq!(output, "stdout", "after_tool_output must not mutate");
}
