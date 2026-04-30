//! 集成测试：消费 codex apply-patch fixtures，验证 parser 与上游同源。
//!
//! 仅覆盖 parser 层职责：
//! - 008_rejects_empty_update_hunk / 013_rejects_invalid_hunk_header 必须在 parse 层报错
//! - 其余可解析的 patch.txt（不管 apply 是否成功）必须 parse 通过
//!
//! 005 与 006 的 reject 发生在 apply 层（patch 自身语法合法），由后续 apply crate 覆盖。

use std::fs;
use std::path::PathBuf;

use dasclaw_apply_patch::{parse_patch, ParseError};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../codex-cli-main/codex-rs/apply-patch/tests/fixtures/scenarios")
}

fn read_patch(scenario: &str) -> String {
    let path = fixtures_root().join(scenario).join("patch.txt");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read fixture {}: {err}", path.display()))
}

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
