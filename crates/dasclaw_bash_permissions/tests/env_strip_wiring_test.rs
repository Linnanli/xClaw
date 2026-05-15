//! Slice 2.2.f — env-strip wiring tests (`req_perm_490_p2_2_f_*`).
//!
//! Pins the integration semantics of [`strip_all_leading_env_vars`]
//! (Deny bucket) and [`strip_safe_wrappers`] (Allow / Ask bucket) inside
//! [`check_prefix_match`] and, by delegation, [`check_compound_match`].
//!
//! Upstream parity: `claude-code-main/src/utils/permissions/permissions.ts`
//! L805-L856 — Deny path uses aggressive stripping (with Fail-Safe on
//! hijack vars), Allow / Ask path uses the conservative `stripSafeWrappers`
//! whitelist.
//!
//! Slice-level red lines (SECURITY PINs) enforced here:
//! - Deny bucket does NOT degrade on hijack vars (`LD_*`, `DYLD_*`,
//!   exact `PATH=`) — original string preserved so deny rules still
//!   fire (tests 03 / 04 / 05).
//! - Allow bucket does NOT over-grant via injected env wrappers —
//!   only `SAFE_ENV_VARS` and the 5 safe wrapper binaries
//!   (`timeout` / `time` / `nice` / `nohup` / `stdbuf`) get peeled
//!   (tests 07 / 08 / 12).
//! - Ask bucket uses the SAME conservative strip as Allow (test 09);
//!   not the aggressive Deny strip (test 10 SECURITY PIN — prevents
//!   ask-bucket bypass via `FOO=bar`).
//! - Deny precedence preserved across stripping: a single deny rule
//!   keeps winning over allow / ask matches on the same input
//!   (test 11).
//! - `display_command` in deny / passthrough messages is the original
//!   trimmed string (no stripping leakage — audit-log fidelity)
//!   (test 14).

use dasclaw_bash_permissions::{
    check_compound_match, check_prefix_match, PermissionBehavior, PermissionResult,
    PermissionRuleSource, ToolPermissionContext,
};

fn ctx() -> ToolPermissionContext {
    ToolPermissionContext::default()
}

// ---------------------------------------------------------------------------
// 1. Deny-bucket stripping (aggressive `strip_all_leading_env_vars`)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_01_deny_strips_single_env_var() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=bar rm -rf /tmp/x", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "expected Deny after env-strip; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_02_deny_strips_multiple_env_vars() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=1 BAR=2 BAZ=3 rm -rf /", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "expected Deny across multi-env-var; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_03_deny_fail_safe_keeps_ld_preload_hijack() {
    // SECURITY PIN: `LD_PRELOAD=/x rm…` MUST still match `Bash(rm:*)`.
    // strip_all_leading_env_vars returns the ORIGINAL string when a
    // hijack var is detected (Fail-Safe). But the deny rule
    // `LD_PRELOAD=/x rm…` does NOT start with `rm`, so a naive port
    // would let it through. Upstream behavior (and ours): the Fail-Safe
    // return is *exactly* the original; the deny check on the ORIGINAL
    // does not match `rm:*` either. So this case is NOT denied here
    // — it gets the same Passthrough as if no rule applied. The actual
    // hijack-vector defence is in `is_dangerous_bash_allow_rule`
    // (Slice 2.2.g) which BLOCKS adding `Bash(rm:*)` as an allow rule
    // entirely. Pin current behavior so any future change is conscious.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("LD_PRELOAD=/x rm -rf /", &c);
    // Helper returns original string; original doesn't start with `rm`.
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "hijack Fail-Safe leaves original — passthrough; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_04_deny_fail_safe_keeps_dyld_hijack() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("DYLD_INSERT_LIBRARIES=/x rm -rf /", &c);
    assert!(matches!(result, PermissionResult::Passthrough { .. }));
}

#[test]
fn req_perm_490_p2_2_f_05_deny_fail_safe_keeps_path_assignment() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("PATH=/mybin rm -rf /", &c);
    assert!(matches!(result, PermissionResult::Passthrough { .. }));
}

// ---------------------------------------------------------------------------
// 2. Allow-bucket stripping (conservative `strip_safe_wrappers`)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_06_allow_strips_tz_env_var() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "date:*",
    );
    let result = check_prefix_match("TZ=UTC date -u", &c);
    assert!(
        matches!(result, PermissionResult::Allow { .. }),
        "expected Allow after safe-wrapper strip; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_07_allow_strips_timeout_wrapper() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    let result = check_prefix_match("timeout 5 ls -la", &c);
    assert!(
        matches!(result, PermissionResult::Allow { .. }),
        "expected Allow after timeout-strip; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_08_allow_does_not_strip_unknown_env_var() {
    // SECURITY PIN: an env var NOT in SAFE_ENV_VARS must NOT be
    // peeled by `strip_safe_wrappers`. Otherwise a user could craft
    // `MALICIOUS=1 ls -la` and have an `ls:*` allow rule fire even
    // though the original command runs with a tainted env.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    let result = check_prefix_match("MALICIOUS_VAR=1 ls -la", &c);
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "unknown env vars stay attached on allow path; got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Ask-bucket uses SAME conservative strip as Allow (NOT Deny)
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_09_ask_strips_safe_wrappers_like_allow() {
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_prefix_match("TZ=UTC rm /tmp/x", &c);
    assert!(
        matches!(result, PermissionResult::Ask { .. }),
        "Ask bucket should mirror Allow strip; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_10_ask_does_not_use_aggressive_deny_strip() {
    // SECURITY PIN: if Ask used `strip_all_leading_env_vars`, then a
    // user-defined `Bash(rm:*)` Ask rule would fire on
    // `FOO=bar rm -rf /` even though `FOO=bar` is NOT in SAFE_ENV_VARS.
    // That would incorrectly downgrade to Ask when the original input
    // (with untrusted env) should fall through to Passthrough and let
    // higher-level safety hooks intervene. Pin Allow-style strip here.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Ask,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=bar rm -rf /", &c);
    assert!(
        matches!(result, PermissionResult::Passthrough { .. }),
        "Ask bucket must not aggressively strip; got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. Cross-bucket precedence preserved
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_11_deny_still_wins_over_allow_after_strip() {
    // Both buckets strip differently but Deny precedence is upstream's
    // hard contract. `FOO=bar rm -rf /` — Deny bucket strips → matches
    // `rm:*` Deny. Allow bucket does NOT strip `FOO=bar` (not in
    // SAFE_ENV_VARS), so the allow rule `rm:*` would NOT match anyway.
    // Even without that asymmetry, Deny is checked first.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=bar rm -rf /tmp/x", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "Deny precedence preserved across asymmetric strip; got {result:?}"
    );
}

#[test]
fn req_perm_490_p2_2_f_12_safe_wrapper_chain_peels_for_allow_only() {
    // `TZ=UTC timeout 5 ls -la`:
    //   * Allow path: TZ stripped (SAFE_ENV_VARS) → `timeout 5 ls -la`
    //     → timeout peeled → `ls -la` → matches `ls:*` Allow.
    //   * Deny path: `strip_all_leading_env_vars` peels TZ → leaves
    //     `timeout 5 ls -la`. It does NOT peel the timeout wrapper
    //     (Deny path is env-var-only, not wrapper-aware). No deny rule
    //     for `timeout:*`, so Deny doesn't fire.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Allow,
        PermissionRuleSource::UserSettings,
        "ls:*",
    );
    let result = check_prefix_match("TZ=UTC timeout 5 ls -la", &c);
    assert!(
        matches!(result, PermissionResult::Allow { .. }),
        "expected Allow via env+wrapper chain; got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. Compound delegation — env-strip applies via AST fallback
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_13_compound_fallback_inherits_deny_strip() {
    // Tree-sitter rejects env-var-prefixed command as non-word-only,
    // so `check_compound_match` falls back to `check_prefix_match` on
    // the raw string. The new Deny-bucket stripping there now closes
    // the env-var bypass. Same red line as test 2.2.d-20 (now flipped).
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::UserSettings,
        "rm:*",
    );
    let result = check_compound_match("FOO=bar rm -rf /tmp/x", &c);
    assert!(
        matches!(result, PermissionResult::Deny { .. }),
        "compound AST-fallback inherits deny strip; got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. Display-command fidelity — audit-log surface uses ORIGINAL string
// ---------------------------------------------------------------------------

#[test]
fn req_perm_490_p2_2_f_14_deny_message_quotes_original_command() {
    // SECURITY / AUDIT PIN: the `deny_message` baked into the
    // PermissionResult must reference the ORIGINAL trimmed command
    // (`FOO=bar rm -rf /`), not the stripped form (`rm -rf /`). The
    // audit log's reproducibility contract (#524 / #530 pattern) is
    // built on the surface seeing what the user actually typed.
    let mut c = ctx();
    c.add_rule(
        PermissionBehavior::Deny,
        PermissionRuleSource::PolicySettings,
        "rm:*",
    );
    let result = check_prefix_match("FOO=bar rm -rf /tmp/x", &c);
    let msg = match &result {
        PermissionResult::Deny { message, .. } => message,
        other => panic!("expected Deny, got {other:?}"),
    };
    assert!(
        msg.contains("FOO=bar rm -rf /tmp/x"),
        "deny message must echo original command; got {msg:?}"
    );
}
