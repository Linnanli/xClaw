//! Phase 2.1 follow-up — audit log contract for `BashValidationHook`
//! security gate.
//!
//! Slice 2.1.d shipped the `tracing::warn!(target: "bash_security::block", …)`
//! audit emission, but its in-file companion test
//! (`test_security_audit_log_contains_rule_id`) could only inspect the
//! *model-visible* `reason` string because no `tracing` subscriber was
//! installed. With `tracing-test` already wired into dev-deps (see
//! `lifecycle_trace_log.rs`) we can now pin the structured audit-log
//! contract directly:
//!
//! - Event message contains the `bash_security::block` audit tag.
//! - `rule_id=<SecurityCheckId Debug>` is present and non-empty.
//! - `tool=` and `mode=` fields are populated for downstream correlators.
//! - The active `PermissionMode` slug is recorded *regardless* of whether
//!   the mode itself would bypass other checks (security gate is mode-
//!   independent per plan §3.3 / §6).
//!
//! `DecisionReason::ParseFailure` is declared in
//! `dasclaw_bash_validation::security::types` for forward compatibility but
//! no production code path emits it today — no validator currently surfaces
//! a parse failure as a synthetic check. Pinning that contract is deferred
//! until a Phase 2.2 validator (or the differential CI from plan §12)
//! exercises it.
//!
//! `ControlCharacters` *does* have a production emit path as of PR #519
//! (first early gate, see
//! `crates/dasclaw_bash_validation/src/security/early.rs::validate_control_characters`).
//! `req_security_490_p2_1_d_audit_log_control_chars_surface_rule_id`
//! pins its `rule_id=ControlCharacters` audit surface so pipeline
//! reorders can't silently demote it.
//!
//! Naming: `req_security_490_p2_1_d_audit_log_<scenario>` — same family as
//! the existing Slice 2.1.d e2e tests so coverage tooling can group them.

use std::path::PathBuf;

use dasclaw_hooks::BashValidationHook;
use serde_json::json;
use tracing_test::traced_test;
use x_claw_agent::permissions::PermissionMode;
use x_claw_agent::{SafetyDecision, SafetyHook};

/// Newline-injection sample — same fixture as the inline Slice 2.1.d
/// tests; trips `validate_security` via the `Newlines` /
/// `QuotedNewline` rules.
const INJECTION_CMD: &str = "echo a\nrm -rf /";

async fn fire_bash(mode: PermissionMode, cmd: &str) -> SafetyDecision {
    let hook = BashValidationHook::new(mode, PathBuf::from("/workspace"));
    let mut args = json!({ "command": cmd });
    hook.before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error on string command")
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_1_d_audit_log_emits_block_tag_and_rule_id() {
    let decision = fire_bash(PermissionMode::WorkspaceWrite, INJECTION_CMD).await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "injection must surface as SafetyDecision::Block; got {decision:?}"
    );

    // Audit tag — operators / SIEM filter on this exact string.
    assert!(
        logs_contain("bash_security::block"),
        "audit log must carry the `bash_security::block` event message"
    );
    // `rule_id` must be present and reference a real `SecurityCheckId`
    // variant (not the synthetic `ParseFailure`).
    assert!(
        logs_contain("rule_id="),
        "audit log must include structured `rule_id` field"
    );
    assert!(
        !logs_contain("rule_id=ParseFailure"),
        "non-parse-failure injection must not be labelled ParseFailure"
    );
    // Correlation fields. `tracing-test` performs substring matching, so
    // accept either `tool=bash` or `tool="bash"` depending on tracing's
    // rendering for `&str` fields.
    assert!(
        logs_contain("tool=") && logs_contain("bash"),
        "audit log must carry the originating tool name"
    );
    assert!(
        logs_contain("workspace-write"),
        "audit log must carry the active PermissionMode slug"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_1_d_audit_log_records_mode_under_full_access() {
    // The security gate is mode-independent: even `DangerFullAccess`
    // must Block injections, and the audit log must record the active
    // mode so post-incident review can prove the operator was in a
    // permissive mode at the time of the refused call.
    let decision = fire_bash(PermissionMode::DangerFullAccess, INJECTION_CMD).await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "injection must Block even in DangerFullAccess; got {decision:?}"
    );

    assert!(
        logs_contain("bash_security::block"),
        "audit tag must fire regardless of PermissionMode"
    );
    assert!(
        logs_contain("danger-full-access"),
        "audit log must carry the active PermissionMode slug even when bypassing permissions"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_1_d_audit_log_silent_on_safe_command() {
    let decision = fire_bash(PermissionMode::WorkspaceWrite, "pwd").await;
    assert!(
        matches!(decision, SafetyDecision::Allow),
        "trivially safe command must not Block; got {decision:?}"
    );
    // No audit event must be emitted on the allow path — the audit tag
    // is exclusively for refused commands so SIEM rules can alert on it
    // without false positives.
    assert!(
        !logs_contain("bash_security::block"),
        "audit tag must only fire on Block, never on Allow"
    );
}

#[tokio::test]
#[traced_test]
async fn req_security_490_p2_1_d_audit_log_control_chars_surface_rule_id() {
    // PR #519 (Slice 2.1.e prerequisite) added `validate_control_characters`
    // as the *first* gate in `validate_security`. Pin its audit-log
    // surface so future refactors of the pipeline order can't silently
    // demote it without breaking this test.
    //
    // Input carries a literal null byte — caught only by the
    // control-character gate; every later validator either ignores
    // non-printable bytes or runs out of order.
    let decision = fire_bash(PermissionMode::WorkspaceWrite, "echo safe\u{0}value").await;
    assert!(
        matches!(decision, SafetyDecision::Block { .. }),
        "null byte must Block; got {decision:?}"
    );

    assert!(
        logs_contain("bash_security::block"),
        "audit tag must fire on control-character refusal"
    );
    // Exact rule_id — protects against pipeline-reorder regressions that
    // would let a later (less specific) validator claim the rule_id.
    assert!(
        logs_contain("rule_id=ControlCharacters"),
        "control-character refusal must surface `rule_id=ControlCharacters`, not a substitute"
    );
    assert!(
        logs_contain("workspace-write"),
        "audit log must carry the active PermissionMode slug"
    );
}
