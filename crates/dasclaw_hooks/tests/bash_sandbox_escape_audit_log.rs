//! Phase 3.2.E.rest (issue #603) — sandbox-escape audit-log contract
//! pinning for [`BashPermissionHook`].
//!
//! Companion to [`bash_permission_audit_log.rs`] (Slice 2.2.h), this
//! file pins the audit-log shape for the two pre-rule-pipeline
//! hard-deny reasons added by 3.2.E.rest:
//!
//! * SECURITY pin **S21** — `xargs` target / flag not in the safe
//!   allowlist (renders as `rule_id=FlagNotInAllowlist`).
//! * SECURITY pin **S22** — write into a git-internal path
//!   (`HEAD` / `objects/` / `refs/` / `hooks/`) inside a git-bearing
//!   compound (renders as `rule_id=GitInternalPathWrite`).
//!
//! ## Why substring pinning, not `insta`
//!
//! The existing audit-log contract in
//! [`bash_permission_audit_log.rs`] / [`bash_security_audit_log.rs`]
//! uses `tracing_test::logs_contain` with exact literal substrings.
//! That is the established convention. Adding `insta` here purely
//! for two new variants would be patch-style accretion. The PIN
//! semantics are identical: any rename of an enum variant, audit
//! event name, or message format flips a hard `assert!` here.
//!
//! ## Coverage matrix
//!
//! | Pin | Rule context | Command                                    | Expected decision    |
//! |-----|--------------|--------------------------------------------|----------------------|
//! | S21 | empty (no rules) | `xargs rm`                              | Block FlagNotInAllowlist |
//! | S21 | allow rule `xargs:*` matches | `xargs -I {} rm`            | Block FlagNotInAllowlist (rule cannot allow) |
//! | S21 | empty            | `xargs echo`                            | Allow (safe target)     |
//! | S22 | empty            | `mkdir -p hooks && echo m > hooks/x && git status` | Block GitInternalPathWrite |
//! | S22 | allow rule `Bash(*)` matches | same as above                   | Block GitInternalPathWrite (rule cannot allow) |
//! | S22 | empty            | `echo m > hooks/x` (no git)             | Passthrough (no git in compound) |
//! | S22 | empty            | `git status && mkdir -p HEAD` (creates HEAD dir) | Block GitInternalPathWrite |
//!
//! The "rule cannot allow" pins are the SECURITY core of this slice:
//! they prove the pre-pipeline pre-check fires regardless of user
//! rules — no admin / project / user config can re-enable a
//! sandbox-escape.

use dasclaw_bash_permissions::{PermissionBehavior, PermissionRuleSource, ToolPermissionContext};
use dasclaw_core::EgressDecision;
use dasclaw_hooks::BashPermissionHook;
use serde_json::json;
use tracing_test::traced_test;

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

async fn fire(ctx: ToolPermissionContext, tool: &str, command: &str) -> EgressDecision {
    let hook = BashPermissionHook::new(ctx);
    let args = json!({ "command": command });
    hook.validate_tool_call(tool, &args)
}

fn block_reason(decision: &EgressDecision) -> &str {
    match decision {
        EgressDecision::Block { reason, .. } => reason,
        other => panic!("expected EgressDecision::Block, got {other:?}"),
    }
}

// =====================================================================
// S21 — `xargs` target / flag not in safe allowlist
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_603_s21_xargs_unsafe_target_block() {
    let decision = fire(empty_ctx(), "bash", "xargs rm").await;
    let reason = block_reason(&decision);

    assert!(
        reason.contains("FlagNotInAllowlist"),
        "block reason must surface variant tag; got {reason}"
    );
    assert!(
        reason.contains("xargs rm"),
        "block reason must echo the offending command; got {reason}"
    );

    assert!(
        logs_contain("bash_perm::block"),
        "audit log must carry `bash_perm::block` event-message tag"
    );
    assert!(
        logs_contain("rule_id=FlagNotInAllowlist"),
        "audit log must surface `rule_id=FlagNotInAllowlist`"
    );
    assert!(
        logs_contain("bash sandbox escape S21"),
        "audit `message` field must pin the human-readable S21 tag"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_603_s21_xargs_unsafe_target_rule_cannot_allow() {
    // SECURITY core: even with a permission rule that would normally
    // allow `xargs ...`, the sandbox-escape pre-check fires first and
    // blocks. No rule config can re-enable S21.
    let ctx = ctx_with(PermissionBehavior::Allow, "xargs:*");
    let decision = fire(ctx, "bash", "xargs -I {} rm").await;

    assert!(
        matches!(decision, EgressDecision::Block { .. }),
        "user allow-rule must NOT bypass S21 hard-deny; got {decision:?}"
    );
    assert!(
        logs_contain("rule_id=FlagNotInAllowlist"),
        "audit log must surface the sandbox-escape rule_id, not the rule pattern"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_603_s21_xargs_safe_target_passes_to_rule_engine() {
    // Negative pin: `xargs echo` is in the safe allowlist; the
    // sandbox-escape pre-check must NOT fire, leaving the decision to
    // the rule engine (which, with no rules, returns Passthrough).
    let decision = fire(empty_ctx(), "bash", "xargs echo hi").await;
    assert!(
        matches!(decision, EgressDecision::Passthrough),
        "safe xargs target must fall through to rule engine; got {decision:?}"
    );
    assert!(
        !logs_contain("bash_perm::block"),
        "no block event must fire for safe xargs invocations"
    );
    assert!(
        !logs_contain("FlagNotInAllowlist"),
        "FlagNotInAllowlist must not appear for safe xargs invocations"
    );
}

// =====================================================================
// S22 — write into git-internal path inside git-bearing compound
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_603_s22_redirect_into_hooks_block() {
    // Canonical S22 from `command_writes_to_git_internal_paths` docs:
    // bare-repo masquerade via `hooks/pre-commit` + `git status`.
    let cmd = "mkdir -p hooks && echo m > hooks/pre-commit && git status";
    let decision = fire(empty_ctx(), "bash", cmd).await;
    let reason = block_reason(&decision);

    assert!(
        reason.contains("GitInternalPathWrite"),
        "block reason must surface variant tag; got {reason}"
    );
    assert!(
        reason.contains("hooks/pre-commit") || reason.contains(cmd),
        "block reason must echo command details; got {reason}"
    );

    assert!(
        logs_contain("rule_id=GitInternalPathWrite"),
        "audit log must surface `rule_id=GitInternalPathWrite`"
    );
    assert!(
        logs_contain("bash sandbox escape S22"),
        "audit `message` field must pin the human-readable S22 tag"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_603_s22_rule_cannot_allow() {
    // SECURITY core: a wildcard allow rule must not let S22 through.
    let ctx = ctx_with(PermissionBehavior::Allow, "*");
    let cmd = "mkdir -p hooks && echo m > hooks/pre-commit && git status";
    let decision = fire(ctx, "bash", cmd).await;

    assert!(
        matches!(decision, EgressDecision::Block { .. }),
        "wildcard allow-rule must NOT bypass S22 hard-deny; got {decision:?}"
    );
    assert!(
        logs_contain("rule_id=GitInternalPathWrite"),
        "audit log must surface sandbox-escape rule_id, not the rule pattern"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_603_s22_no_git_in_compound_passes_through() {
    // Negative pin: write into `hooks/` WITHOUT any git invocation in
    // the compound is not the bare-repo masquerade attack (it's just a
    // file write to an oddly-named directory). The sandbox-escape
    // pre-check must NOT fire; the rule engine decides.
    let decision = fire(empty_ctx(), "bash", "echo m > hooks/pre-commit").await;
    assert!(
        matches!(decision, EgressDecision::Passthrough),
        "non-git hooks/ write must fall through to rule engine; got {decision:?}"
    );
    assert!(
        !logs_contain("GitInternalPathWrite"),
        "GitInternalPathWrite must not appear when no git is in the compound"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_603_s22_mkdir_head_dir_block() {
    // Vector-2 surface: `mkdir HEAD` creates a top-level HEAD file/dir
    // that git will then read as bare-repo HEAD. Compound also
    // contains a git invocation, so the pre-check fires.
    let decision = fire(empty_ctx(), "bash", "git status && mkdir -p HEAD").await;
    let reason = block_reason(&decision);
    assert!(
        reason.contains("GitInternalPathWrite"),
        "mkdir into HEAD inside git-bearing compound must block; got {reason}"
    );
}

// =====================================================================
// Precedence — S22 wins over S21 when both apply
// =====================================================================

#[tokio::test]
#[traced_test]
async fn req_security_603_s22_precedes_s21() {
    // `xargs git commit` is an unsafe xargs target (`git` not in
    // SAFE_TARGET_COMMANDS_FOR_XARGS), AND the compound writes to
    // `hooks/`. The pre-check checks S22 first so the more-specific
    // git-internal reason wins the audit log.
    let cmd = "xargs git commit && echo m > hooks/pre-commit";
    let decision = fire(empty_ctx(), "bash", cmd).await;
    let reason = block_reason(&decision);
    assert!(
        reason.contains("GitInternalPathWrite"),
        "S22 must take precedence over S21 when both apply; got {reason}"
    );
    assert!(
        !reason.contains("FlagNotInAllowlist"),
        "S21 reason must not also appear in the same audit line; got {reason}"
    );
}
