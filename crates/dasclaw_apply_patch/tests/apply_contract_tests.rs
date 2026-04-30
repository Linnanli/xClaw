//! Contract tests for the `apply` layer (W3 #53).
//!
//! These tests pin the minimal apply contract consumed by ironclaw's
//! `ApplyPatchTool` wrapper:
//!
//! - Add / Delete / Update hunks roundtrip through the filesystem.
//! - `base_dir` enforces relative-path policy (absolute → reject, `..` escape
//!   → reject).
//! - Update hunks tolerate trailing-whitespace and leading/trailing whitespace
//!   drift between patch and source (rstrip / trim fallback), matching codex
//!   `seek_sequence` levels 1–3.
//! - Multi-hunk updates apply in patch order.
//!
//! Out of scope (left to follow-up issues): `Move to:` rename, unicode
//! punctuation normalization, and EOF-anchored fuzzy matching beyond the
//! `is_end_of_file` flag.
//!
//! Patch literals are built via `[..].join("\n")` because Rust's `\` line
//! continuation eats leading whitespace, which would silently corrupt the
//! `" "` (context line) sentinel inside update hunks.

use std::fs;

use dasclaw_apply_patch::{apply, parse_patch, ApplyError, ApplyOptions};
use tempfile::TempDir;

fn opts(base: &TempDir) -> ApplyOptions {
    ApplyOptions {
        base_dir: Some(base.path().to_path_buf()),
    }
}

fn patch(lines: &[&str]) -> String {
    lines.join("\n")
}

#[test]
fn req_w3_53_apply_add_file_creates_with_parents() {
    let base = TempDir::new().unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Add File: nested/dir/hello.txt",
        "+line one",
        "+line two",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let report = apply(&args, &opts(&base)).unwrap();
    let written = fs::read_to_string(base.path().join("nested/dir/hello.txt")).unwrap();
    assert_eq!(written, "line one\nline two\n");
    assert_eq!(report.files_added.len(), 1);
    assert_eq!(report.hunks_applied, 1);
}

#[test]
fn req_w3_53_apply_add_file_rejects_when_target_exists() {
    let base = TempDir::new().unwrap();
    fs::write(base.path().join("clash.txt"), "existing").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Add File: clash.txt",
        "+should not write",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::FileAlreadyExists(_)));
    assert_eq!(
        fs::read_to_string(base.path().join("clash.txt")).unwrap(),
        "existing"
    );
}

#[test]
fn req_w3_53_apply_delete_file_removes_target() {
    let base = TempDir::new().unwrap();
    fs::write(base.path().join("gone.txt"), "bye").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Delete File: gone.txt",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let report = apply(&args, &opts(&base)).unwrap();
    assert!(!base.path().join("gone.txt").exists());
    assert_eq!(report.files_deleted.len(), 1);
}

#[test]
fn req_w3_53_apply_delete_missing_file_errors() {
    let base = TempDir::new().unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Delete File: missing.txt",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::FileNotFound(_)));
}

#[test]
fn req_w3_53_apply_update_file_replaces_lines() {
    let base = TempDir::new().unwrap();
    fs::write(base.path().join("code.txt"), "alpha\nbeta\ngamma\ndelta\n").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Update File: code.txt",
        "@@",
        " alpha",
        "-beta",
        "+BETA",
        " gamma",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    apply(&args, &opts(&base)).unwrap();
    let updated = fs::read_to_string(base.path().join("code.txt")).unwrap();
    assert_eq!(updated, "alpha\nBETA\ngamma\ndelta\n");
}

#[test]
fn req_w3_53_apply_update_tolerates_trailing_whitespace_drift() {
    let base = TempDir::new().unwrap();
    // Source has trailing whitespace; patch context does not. Tier-2 (rstrip)
    // fallback locates the match; the rewrite then normalizes context to the
    // patch's literal form, mirroring standard `git apply` behavior.
    fs::write(base.path().join("ws.txt"), "alpha   \nbeta\n").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Update File: ws.txt",
        "@@",
        " alpha",
        "-beta",
        "+BETA",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    apply(&args, &opts(&base)).unwrap();
    assert_eq!(
        fs::read_to_string(base.path().join("ws.txt")).unwrap(),
        "alpha\nBETA\n"
    );
}

#[test]
fn req_w3_53_apply_update_context_not_found_errors() {
    let base = TempDir::new().unwrap();
    fs::write(base.path().join("code.txt"), "alpha\nbeta\n").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Update File: code.txt",
        "@@",
        " zeta",
        "-beta",
        "+BETA",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::ContextNotFound(_)));
    // File contents must be unchanged on failure.
    assert_eq!(
        fs::read_to_string(base.path().join("code.txt")).unwrap(),
        "alpha\nbeta\n"
    );
}

#[test]
fn req_w3_53_apply_multi_hunk_in_order() {
    let base = TempDir::new().unwrap();
    fs::write(
        base.path().join("multi.txt"),
        "one\ntwo\nthree\nfour\nfive\n",
    )
    .unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Update File: multi.txt",
        "@@",
        " one",
        "-two",
        "+TWO",
        " three",
        "@@",
        " three",
        "-four",
        "+FOUR",
        " five",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    apply(&args, &opts(&base)).unwrap();
    assert_eq!(
        fs::read_to_string(base.path().join("multi.txt")).unwrap(),
        "one\nTWO\nthree\nFOUR\nfive\n"
    );
}

#[test]
fn req_w3_53_apply_rejects_absolute_path_under_base() {
    let base = TempDir::new().unwrap();
    let abs_line = format!("*** Add File: {}/abs.txt", base.path().display());
    let p = patch(&[
        "*** Begin Patch",
        abs_line.as_str(),
        "+nope",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::AbsolutePath(_)));
}

#[test]
fn req_w3_53_apply_rejects_parent_dir_escape() {
    let base = TempDir::new().unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Add File: ../escape.txt",
        "+nope",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::PathEscapesBase(_)));
}

#[test]
fn req_w3_53_apply_move_not_supported_yet() {
    // `Move to:` is parsed but not yet applied — pinned as ApplyError so a
    // future PR upgrading the apply layer flips this to a positive test.
    let base = TempDir::new().unwrap();
    fs::write(base.path().join("a.txt"), "x\n").unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Update File: a.txt",
        "*** Move to: b.txt",
        "@@",
        "-x",
        "+y",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let err = apply(&args, &opts(&base)).unwrap_err();
    assert!(matches!(err, ApplyError::MoveNotSupported));
}

#[test]
fn req_w3_53_apply_no_base_trusts_caller_for_sandboxing() {
    // Without `base_dir`, the apply layer trusts the caller (ironclaw's tool
    // wrapper enforces base_dir/policy itself). A clean tempdir + chdir makes
    // the test hermetic; parser appends a trailing newline to add-file
    // contents per the lark grammar.
    let base = TempDir::new().unwrap();
    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(base.path()).unwrap();
    let p = patch(&[
        "*** Begin Patch",
        "*** Add File: rel.txt",
        "+ok",
        "*** End Patch",
    ]);
    let args = parse_patch(&p).unwrap();
    let result = apply(&args, &ApplyOptions::default());
    std::env::set_current_dir(cwd).unwrap();
    result.unwrap();
    assert_eq!(
        fs::read_to_string(base.path().join("rel.txt")).unwrap(),
        "ok\n"
    );
}
