//! 集成测试：消费 codex apply-patch fixtures，验证 parser 与上游同源。
//!
//! Fixtures 已完整复制到 `tests/fixtures/scenarios/`（W3 #62），与上游
//! `codex-cli-main/codex-rs/apply-patch/tests/fixtures/scenarios/` 同源。
//! 复制保证测试自包含，不再依赖 vendored codex-cli 目录的相对路径。
//!
//! 仅覆盖 parser 层职责：
//! - 008_rejects_empty_update_hunk / 013_rejects_invalid_hunk_header 必须在 parse 层报错
//! - 其余可解析的 patch.txt（不管 apply 是否成功）必须 parse 通过
//!
//! 005 / 006 / 007 / 009 / 010 / 011 / 012 / 015 的 reject 发生在 apply 层
//! （patch 自身语法合法），由后续 apply crate 覆盖。

use std::fs;
use std::path::PathBuf;

use dasclaw_apply_patch::{parse_patch, ParseError};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scenarios")
}

fn read_patch(scenario: &str) -> String {
    let path = fixtures_root().join(scenario).join("patch.txt");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()))
}

// ---------------------------------------------------------------------------
// W3 #52 — initial subset (kept verbatim; fixtures path now resolves locally).
// ---------------------------------------------------------------------------

#[test]
fn req_w3_52_codex_fixture_002_multiple_operations_parses() {
    parse_patch(&read_patch("002_multiple_operations")).expect("multi-op fixture parses");
}

#[test]
fn req_w3_52_codex_fixture_003_multiple_chunks_parses() {
    parse_patch(&read_patch("003_multiple_chunks")).expect("multi-chunk fixture parses");
}

#[test]
fn req_w3_52_codex_fixture_004_move_to_new_directory_parses() {
    parse_patch(&read_patch("004_move_to_new_directory")).expect("move fixture parses");
}

#[test]
fn req_w3_52_codex_fixture_008_rejects_empty_update_hunk() {
    let err = parse_patch(&read_patch("008_rejects_empty_update_hunk"))
        .expect_err("empty update hunk must be rejected at parse layer");
    assert!(
        matches!(err, ParseError::InvalidHunkError { .. }),
        "expected InvalidHunkError, got {err:?}",
    );
}

#[test]
fn req_w3_52_codex_fixture_013_rejects_invalid_hunk_header() {
    let err = parse_patch(&read_patch("013_rejects_invalid_hunk_header"))
        .expect_err("invalid hunk header must be rejected at parse layer");
    assert!(
        matches!(err, ParseError::InvalidHunkError { .. }),
        "expected InvalidHunkError, got {err:?}",
    );
}

#[test]
fn req_w3_52_codex_fixture_014_update_file_appends_trailing_newline_parses() {
    parse_patch(&read_patch("014_update_file_appends_trailing_newline"))
        .expect("trailing newline fixture parses");
}

#[test]
fn req_w3_52_codex_fixture_017_whitespace_padded_hunk_header_parses() {
    parse_patch(&read_patch("017_whitespace_padded_hunk_header"))
        .expect("whitespace-padded hunk header parses");
}

#[test]
fn req_w3_52_codex_fixture_018_whitespace_padded_patch_markers_parses() {
    parse_patch(&read_patch("018_whitespace_padded_patch_markers"))
        .expect("whitespace-padded patch markers parse");
}

#[test]
fn req_w3_52_codex_fixture_019_unicode_simple_parses() {
    parse_patch(&read_patch("019_unicode_simple")).expect("unicode fixture parses");
}

#[test]
fn req_w3_52_codex_fixture_022_update_file_end_of_file_marker_parses() {
    let parsed = parse_patch(&read_patch("022_update_file_end_of_file_marker"))
        .expect("eof marker fixture parses");
    assert!(!parsed.hunks.is_empty(), "expected at least one hunk");
}

// ---------------------------------------------------------------------------
// W3 #62 — port the remaining 13 scenarios so the full upstream suite runs.
// Apply-layer reject scenarios (005/006/007/009/010/011/012/015) must parse
// cleanly here; their rejection is the apply crate's responsibility.
// ---------------------------------------------------------------------------

#[test]
fn req_w3_62_codex_fixture_001_add_file_parses() {
    let parsed = parse_patch(&read_patch("001_add_file")).expect("add-file fixture parses");
    assert!(!parsed.hunks.is_empty(), "expected at least one hunk");
}

#[test]
fn req_w3_62_codex_fixture_005_empty_patch_parses_with_no_hunks() {
    let parsed = parse_patch(&read_patch("005_rejects_empty_patch"))
        .expect("empty patch parses (apply layer rejects)");
    assert!(
        parsed.hunks.is_empty(),
        "empty patch must produce zero hunks, got {} hunks",
        parsed.hunks.len()
    );
}

#[test]
fn req_w3_62_codex_fixture_006_missing_context_parses() {
    parse_patch(&read_patch("006_rejects_missing_context"))
        .expect("missing-context fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_007_missing_file_delete_parses() {
    parse_patch(&read_patch("007_rejects_missing_file_delete"))
        .expect("missing-file delete fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_009_missing_file_update_parses() {
    parse_patch(&read_patch("009_requires_existing_file_for_update"))
        .expect("missing-file update fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_010_move_overwrites_existing_destination_parses() {
    parse_patch(&read_patch("010_move_overwrites_existing_destination"))
        .expect("move-overwrite fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_011_add_overwrites_existing_file_parses() {
    parse_patch(&read_patch("011_add_overwrites_existing_file"))
        .expect("add-overwrite fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_012_delete_directory_fails_parses() {
    parse_patch(&read_patch("012_delete_directory_fails"))
        .expect("delete-directory fixture parses (apply layer rejects)");
}

#[test]
fn req_w3_62_codex_fixture_015_failure_after_partial_success_parses() {
    parse_patch(&read_patch(
        "015_failure_after_partial_success_leaves_changes",
    ))
    .expect("partial-success fixture parses (apply layer rejects later op)");
}

#[test]
fn req_w3_62_codex_fixture_016_pure_addition_update_chunk_parses() {
    parse_patch(&read_patch("016_pure_addition_update_chunk"))
        .expect("pure-addition update chunk parses");
}

#[test]
fn req_w3_62_codex_fixture_020_delete_file_success_parses() {
    parse_patch(&read_patch("020_delete_file_success")).expect("delete-success fixture parses");
}

#[test]
fn req_w3_62_codex_fixture_020_whitespace_padded_patch_marker_lines_parses() {
    parse_patch(&read_patch("020_whitespace_padded_patch_marker_lines"))
        .expect("whitespace-padded marker lines parse");
}

#[test]
fn req_w3_62_codex_fixture_021_update_file_deletion_only_parses() {
    parse_patch(&read_patch("021_update_file_deletion_only"))
        .expect("update deletion-only fixture parses");
}
