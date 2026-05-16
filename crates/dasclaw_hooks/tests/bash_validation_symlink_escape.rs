//! Phase 3.1.g.2 (ADR-150 Step 6.2) — `BashValidationHook` symlink-escape
//! integration tests.
//!
//! Phase 3.1.A (PR #574) shipped the lexical path-gate; Phase 3.1.g.1
//! (PR #581) shipped the `FsResolver` trait + `RealFsResolver` POSIX
//! implementation + walker. Step 6.2 is the wireup: the hook's
//! `before_tool_call` now invokes
//! [`dasclaw_bash_validation::check_path_constraints_with_fs`] with the
//! real POSIX resolver on Unix, closing the Fail-Open on symlink escape
//! that 3.1.A left behind.
//!
//! These tests are deliberately *hook-level integration tests* (not
//! `validate_command_paths_with_fs` unit tests — those already live in
//! `crates/dasclaw_bash_validation/src/path_validation.rs::symlink_escape`).
//! They pin the seam between the hook adapter and the resolver-aware
//! path-gate, including:
//!
//! - canonical-workspace handling (macOS `/tmp` ↔ `/private/tmp`),
//! - `PermissionMode` → `SafetyDecision` mapping for path-gate `Ask`,
//! - bash AST → argv → resolver round-trip preserves the chain reason.
//!
//! Naming follows the `req_safety_490_3_1_g_2_hook_*` family so coverage
//! tooling groups them with the parent ADR-150 PIN set.

#![cfg(unix)]

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use dasclaw_hooks::BashValidationHook;
use serde_json::json;
use tempfile::TempDir;
use x_claw_agent::permissions::PermissionMode;
use x_claw_agent::{SafetyDecision, SafetyHook};

/// Build a tempdir + canonical workspace root pair. Canonicalization is
/// **the test's job** here only so we control the comparison anchor;
/// `BashValidationHook::new` canonicalizes on its own so production
/// callers don't have to.
fn ws() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let root = std::fs::canonicalize(dir.path()).expect("canonicalize tempdir");
    (dir, root)
}

/// Build a hook rooted at `ws_root` in `Prompt` mode (so path-gate `Ask`
/// surfaces as `SafetyDecision::Ask` rather than being folded into
/// `Block` / `Allow` by the permission matrix). Home is set to a hermetic
/// non-existent path to make `~` expansion deterministic across hosts.
fn hook_at(ws_root: &Path) -> BashValidationHook {
    BashValidationHook::new(PermissionMode::Prompt, ws_root.to_path_buf())
        .with_home_dir(Some(PathBuf::from("/nonexistent-home")))
}

async fn fire(hook: &BashValidationHook, command: &str) -> SafetyDecision {
    let mut args = json!({ "command": command });
    hook.before_tool_call("bash", &mut args)
        .await
        .expect("hook must not error on a string command")
}

// ---------------------------------------------------------------------
// T-SYM-1: workspace-local symlink to /etc/passwd → Ask via path-gate
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_t_sym_1_link_to_etc_passwd_asks() {
    let (_dir, ws_root) = ws();
    let link = ws_root.join("leak");
    symlink("/etc/passwd", &link).expect("create symlink");

    let h = hook_at(&ws_root);
    let cmd = format!("cat {}", link.display());
    let decision = fire(&h, &cmd).await;

    match decision {
        SafetyDecision::Ask { reason, .. } => {
            assert!(
                reason.starts_with("bash_path_constraints::ask"),
                "expected path-gate Ask prefix, got: {reason}"
            );
            assert!(
                reason.contains("/etc/passwd"),
                "Ask reason must cite the escaping chain step (/etc/passwd), got: {reason}"
            );
            assert!(
                reason.contains("symlink"),
                "Ask reason must mention symlink resolution, got: {reason}"
            );
        }
        other => panic!("expected Ask, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// T-SYM-3: multi-hop chain ending outside workspace → Ask
// (workspace-internal symlink → another workspace-internal symlink →
//  /etc/passwd)
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_t_sym_3_multi_hop_escape_asks() {
    let (_dir, ws_root) = ws();
    let hop_a = ws_root.join("a");
    let hop_b = ws_root.join("b");
    symlink(&hop_b, &hop_a).expect("a -> b");
    symlink("/etc/passwd", &hop_b).expect("b -> /etc/passwd");

    let h = hook_at(&ws_root);
    let cmd = format!("cat {}", hop_a.display());
    let decision = fire(&h, &cmd).await;

    match decision {
        SafetyDecision::Ask { reason, .. } => {
            assert!(reason.contains("/etc/passwd"), "got: {reason}");
        }
        other => panic!("expected Ask for multi-hop escape, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// T-SYM-4: A↔B loop → Ask (chain truncated)
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_t_sym_4_loop_asks_with_chain_truncated() {
    let (_dir, ws_root) = ws();
    let a = ws_root.join("loop_a");
    let b = ws_root.join("loop_b");
    symlink(&b, &a).expect("a -> b");
    symlink(&a, &b).expect("b -> a");

    let h = hook_at(&ws_root);
    let cmd = format!("cat {}", a.display());
    let decision = fire(&h, &cmd).await;

    match decision {
        SafetyDecision::Ask { reason, .. } => {
            assert!(
                reason.contains("symlink chain truncated"),
                "expected truncation reason, got: {reason}"
            );
        }
        other => panic!("expected Ask for symlink loop, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Plain in-workspace path → Passthrough → `validate_command` decides.
// This pins the no-regression invariant vs 3.1.C: ordinary reads
// inside the workspace must not be perturbed by the resolver wireup.
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_plain_in_workspace_path_allows() {
    let (_dir, ws_root) = ws();
    let file = ws_root.join("README.md");
    std::fs::write(&file, b"hi").expect("write");

    let h = hook_at(&ws_root);
    let cmd = format!("cat {}", file.display());
    let decision = fire(&h, &cmd).await;

    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "plain in-workspace cat must Allow (no regression vs 3.1.C)"
    );
}

// ---------------------------------------------------------------------
// Workspace-internal symlink to another workspace-internal file →
// Passthrough → Allow. Confirms the walker does not over-trigger on
// every symlink, only on chains that escape.
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_internal_symlink_allows() {
    let (_dir, ws_root) = ws();
    let target = ws_root.join("real.txt");
    std::fs::write(&target, b"ok").expect("write");
    let link = ws_root.join("alias");
    symlink(&target, &link).expect("symlink");

    let h = hook_at(&ws_root);
    let cmd = format!("cat {}", link.display());
    let decision = fire(&h, &cmd).await;

    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "internal symlink must not trigger path-gate Ask"
    );
}

// ---------------------------------------------------------------------
// PermissionMode interaction: same T-SYM-1 escape under `ReadOnly`
// must Fail-Safe Block (not Ask), matching `map_path_outcome_by_mode`.
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_symlink_escape_readonly_blocks_fail_safe() {
    let (_dir, ws_root) = ws();
    let link = ws_root.join("leak");
    symlink("/etc/passwd", &link).expect("create symlink");

    let h = BashValidationHook::new(PermissionMode::ReadOnly, ws_root.clone())
        .with_home_dir(Some(PathBuf::from("/nonexistent-home")));
    let cmd = format!("cat {}", link.display());
    let decision = fire(&h, &cmd).await;

    match decision {
        SafetyDecision::Block { reason } => {
            assert!(
                reason.starts_with("bash_path_constraints::ask"),
                "ReadOnly must surface the path-gate reason verbatim, got: {reason}"
            );
            assert!(reason.contains("/etc/passwd"), "got: {reason}");
        }
        other => panic!("expected Block (Fail-Safe), got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Constructor canonicalization: caller passes the un-canonical tempdir
// path (e.g. /tmp/... on macOS, which canonicalizes to /private/tmp/...).
// The hook must canonicalize internally so the chain-step comparison
// matches RealFsResolver's canonical output.
// ---------------------------------------------------------------------
#[tokio::test]
async fn req_safety_490_3_1_g_2_hook_canonicalizes_workspace() {
    let dir = TempDir::new().expect("tempdir");
    let raw = dir.path().to_path_buf();
    let canonical = std::fs::canonicalize(&raw).expect("canonicalize");
    // Sanity-guard the test: if the tempdir is already canonical the
    // assertion below degenerates but stays correct.
    let target = canonical.join("file.txt");
    std::fs::write(&target, b"x").expect("write");

    let h = BashValidationHook::new(PermissionMode::Prompt, raw)
        .with_home_dir(Some(PathBuf::from("/nonexistent-home")));
    let cmd = format!("cat {}", target.display());
    let decision = fire(&h, &cmd).await;

    assert_eq!(
        decision,
        SafetyDecision::Allow,
        "hook must canonicalize workspace so /tmp ↔ /private/tmp matches"
    );
}
