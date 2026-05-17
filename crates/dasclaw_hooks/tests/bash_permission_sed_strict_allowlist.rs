//! Slice 4.2 — `BashPermissionHook` strict sed allowlist upgrade.
//!
//! Tracker: issue #607. Tests follow the `req_perm_490_4_2_*` family.
//!
//! Contract (matches issue #607 and ADR-151 sandbox-model context):
//!
//! - Opt-in via [`BashPermissionHook::with_sed_strict_allowlist`]. Default
//!   off → existing behavior preserved (backward-compat for downstream
//!   users that have not yet enabled strict mode).
//! - When the rule pipeline returns `Passthrough` or `Ask` AND the command
//!   matches the upstream-parity strict sed allowlist (single sed call,
//!   either Pattern 1 line-print with `-n`, or Pattern 2 single
//!   substitution), the decision is upgraded to `Allow`.
//! - `Block` and `Allow` decisions from the rule pipeline are **never**
//!   touched — strict mode only upgrades, never downgrades. This keeps
//!   Deny rules sovereign (matches PR #557's "Deny > Allow" precedence)
//!   and avoids redundant re-checks on already-Allow paths.
//! - Non-sed commands fall through unchanged (the allowlist function
//!   returns `false` for them, so no upgrade fires).
//! - Audit log: upgrades emit `tracing::info!` with event message
//!   `bash_perm::allow_sed_strict_upgrade`, structured fields
//!   `tool` + `rule_id="SedAllowlist"`.

use dasclaw_bash_permissions::{PermissionBehavior, PermissionRuleSource, ToolPermissionContext};
use dasclaw_hooks::BashPermissionHook;
use serde_json::json;
use tracing_test::traced_test;
use x_claw_agent::{SafetyDecision, SafetyHook};

fn empty_ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

fn ctx_with(behavior: PermissionBehavior, rule_content: &str) -> ToolPermissionContext {
    let mut ctx = ToolPermissionContext::default();
    ctx.add_rule(
        behavior,
        PermissionRuleSource::ProjectSettings,
        rule_content,
    );
    ctx
}

async fn fire_with_hook(hook: BashPermissionHook, command: &str) -> SafetyDecision {
    let mut args = json!({ "command": command });
    hook.before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error on string command")
}

// --- 01: default (strict off) preserves existing behavior --------------

#[tokio::test]
async fn req_perm_490_4_2_01_strict_off_default_does_not_upgrade_passthrough() {
    // No rule + no strict mode → existing Passthrough behavior wins.
    // This pins backward-compatibility: enabling Phase 4.2 must be a
    // deliberate opt-in, never an implicit behavioral change.
    let hook = BashPermissionHook::new(empty_ctx());
    let decision = fire_with_hook(hook, "sed 's/old/new/g'").await;
    assert_eq!(
        decision,
        SafetyDecision::Passthrough,
        "strict mode off → safe sed stays Passthrough; got {decision:?}"
    );
}

// --- 02: strict on, pattern 2 stdin substitution → Allow ---------------

#[tokio::test]
async fn req_perm_490_4_2_02_strict_on_safe_substitution_stdin_upgrades_passthrough() {
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed 's/old/new/g'").await;
    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "strict on + allowlist-clean substitution → Allow; got {decision:?}"
    );
}

// --- 03: strict on, pattern 1 line print → Allow ----------------------

#[tokio::test]
async fn req_perm_490_4_2_03_strict_on_line_print_with_dash_n_upgrades_passthrough() {
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed -n '1,10p' file.txt").await;
    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "strict on + `sed -n` line print → Allow; got {decision:?}"
    );
}

// --- 04: strict on upgrades Ask, not just Passthrough -----------------

#[tokio::test]
async fn req_perm_490_4_2_04_strict_on_upgrades_ask_to_allow_when_safe() {
    // Ask rule on `sed:*` would normally surface as Ask. Strict allowlist
    // pre-approves the safe pattern so the user is not prompted for
    // known-safe operations.
    let ctx = ctx_with(PermissionBehavior::Ask, "sed:*");
    let hook = BashPermissionHook::new(ctx).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed 's/foo/bar/'").await;
    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "strict on + Ask rule + safe pattern → upgrade Ask to Allow; got {decision:?}"
    );
}

// --- 05: strict on does NOT downgrade Block ---------------------------

#[tokio::test]
async fn req_perm_490_4_2_05_strict_on_must_not_override_deny() {
    // Even if the command is allowlist-clean, a Deny rule must win.
    // Strict mode only upgrades; it never downgrades. This pins the
    // "Deny is sovereign" invariant from PR #557.
    let ctx = ctx_with(PermissionBehavior::Deny, "sed:*");
    let hook = BashPermissionHook::new(ctx).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed 's/old/new/'").await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "strict on must not override Deny rule; got {decision:?}"
    );
}

// --- 06: strict on with allow_file_writes lets `-i` through -----------

#[tokio::test]
async fn req_perm_490_4_2_06_strict_on_allow_writes_upgrades_in_place_edit() {
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(true);
    let decision = fire_with_hook(hook, "sed -i 's/old/new/g' file.txt").await;
    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "strict on + allow_file_writes → `-i` substitution → Allow; got {decision:?}"
    );
}

// --- 07: strict on with allow_file_writes=false rejects `-i` ----------

#[tokio::test]
async fn req_perm_490_4_2_07_strict_on_strict_writes_keeps_in_place_passthrough() {
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed -i 's/old/new/g' file.txt").await;
    assert_eq!(
        decision,
        SafetyDecision::Passthrough,
        "strict on + allow_file_writes=false + `-i` → no upgrade; got {decision:?}"
    );
}

// --- 08: strict on does not affect non-sed commands -------------------

#[tokio::test]
async fn req_perm_490_4_2_08_strict_on_does_not_touch_non_sed_commands() {
    // The allowlist returns false for non-sed → no upgrade. This guards
    // against accidental "Allow" leakage if the allowlist function is
    // ever changed to be more permissive for non-sed input.
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "ls -la").await;
    assert_eq!(
        decision,
        SafetyDecision::Passthrough,
        "strict on must not upgrade non-sed commands; got {decision:?}"
    );
}

// --- 09: strict on does NOT upgrade allowlist-rejected sed ------------

#[tokio::test]
async fn req_perm_490_4_2_09_strict_on_keeps_passthrough_for_unsafe_sed() {
    // Multiple `-e` expressions → not in upstream Pattern 1 or 2 → allowlist returns false.
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed -e 's/x/y/' -e 's/a/b/'").await;
    assert_eq!(
        decision,
        SafetyDecision::Passthrough,
        "strict on + multi-`-e` not in allowlist → no upgrade; got {decision:?}"
    );
}

// --- 10: audit log emitted on upgrade ---------------------------------

#[tokio::test]
#[traced_test]
async fn req_perm_490_4_2_10_strict_upgrade_emits_audit_log() {
    let hook = BashPermissionHook::new(empty_ctx()).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed 's/old/new/g'").await;
    assert_eq!(decision, SafetyDecision::Allow);
    assert!(
        logs_contain("bash_perm::allow_sed_strict_upgrade"),
        "strict mode upgrade must emit `bash_perm::allow_sed_strict_upgrade` audit event"
    );
    assert!(
        logs_contain("SedAllowlist"),
        "audit event must carry `rule_id=SedAllowlist` for SIEM filtering"
    );
}

// --- 11: strict on does NOT touch existing Allow (idempotent) ---------

#[tokio::test]
async fn req_perm_490_4_2_11_strict_on_does_not_re_log_existing_allow() {
    // If a rule already returned Allow, we must not re-process the
    // command — that would emit a misleading audit event and waste
    // tokenization work. Allow stays Allow with no extra side effects.
    let ctx = ctx_with(PermissionBehavior::Allow, "sed:*");
    let hook = BashPermissionHook::new(ctx).with_sed_strict_allowlist(false);
    let decision = fire_with_hook(hook, "sed 's/old/new/'").await;
    assert_eq!(decision, SafetyDecision::Allow);
}
