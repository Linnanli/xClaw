//! Contract tests for `parse_patch` API.
//!
//! These tests pin the public surface required by W3 Issue #52:
//! - `parse_patch(&str) -> Result<ApplyPatchArgs, ParseError>`
//! - `Hunk::{AddFile, DeleteFile, UpdateFile}`
//! - `UpdateFileChunk` with `change_context`, `old_lines`, `new_lines`, `is_end_of_file`
//! - `ParseError::{InvalidPatchError, InvalidHunkError}`
//!
//! Source spec: codex-cli-main/codex-rs/apply-patch/src/parser.rs (lark grammar).

use std::path::PathBuf;

use dasclaw_apply_patch::parse_patch;
use dasclaw_apply_patch::Hunk;
use dasclaw_apply_patch::ParseError;
use dasclaw_apply_patch::UpdateFileChunk;

#[test]
fn req_w3_52_add_file_hunk_parses() {
    let patch = "*** Begin Patch\n\
                 *** Add File: src/hello.txt\n\
                 +hello\n\
                 +world\n\
                 *** End Patch";
    let args = parse_patch(patch).expect("strict add-file should parse");
    assert_eq!(args.hunks.len(), 1);
    let Hunk::AddFile { path, contents } = &args.hunks[0] else {
        panic!("expected AddFile, got {:?}", args.hunks[0]);
    };
    assert_eq!(path, &PathBuf::from("src/hello.txt"));
    assert_eq!(contents, "hello\nworld\n");
}

#[test]
fn req_w3_52_delete_file_hunk_parses() {
    let patch = "*** Begin Patch\n\
                 *** Delete File: gone.txt\n\
                 *** End Patch";
    let args = parse_patch(patch).expect("strict delete-file should parse");
    assert_eq!(args.hunks.len(), 1);
    let Hunk::DeleteFile { path } = &args.hunks[0] else {
        panic!("expected DeleteFile, got {:?}", args.hunks[0]);
    };
    assert_eq!(path, &PathBuf::from("gone.txt"));
}

#[test]
fn req_w3_52_update_file_with_context_marker_parses() {
    let patch = "*** Begin Patch\n\
                 *** Update File: file.py\n\
                 @@ def f():\n\
                 -    pass\n\
                 +    return 123\n\
                 *** End Patch";
    let args = parse_patch(patch).expect("update with @@ context should parse");
    let Hunk::UpdateFile {
        path,
        move_path,
        chunks,
    } = &args.hunks[0]
    else {
        panic!("expected UpdateFile, got {:?}", args.hunks[0]);
    };
    assert_eq!(path, &PathBuf::from("file.py"));
    assert!(move_path.is_none());
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        chunks[0],
        UpdateFileChunk {
            change_context: Some("def f():".to_string()),
            old_lines: vec!["    pass".to_string()],
            new_lines: vec!["    return 123".to_string()],
            is_end_of_file: false,
        }
    );
}

#[test]
fn req_w3_52_update_with_move_and_eof_marker_parses() {
    let patch = "*** Begin Patch\n\
                 *** Update File: src/old.rs\n\
                 *** Move to: src/new.rs\n\
                 @@\n\
                 -old\n\
                 +new\n\
                 *** End of File\n\
                 *** End Patch";
    let args = parse_patch(patch).expect("update with move + EOF should parse");
    let Hunk::UpdateFile {
        path,
        move_path,
        chunks,
    } = &args.hunks[0]
    else {
        panic!("expected UpdateFile, got {:?}", args.hunks[0]);
    };
    assert_eq!(path, &PathBuf::from("src/old.rs"));
    assert_eq!(
        move_path.as_deref(),
        Some(std::path::Path::new("src/new.rs"))
    );
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].is_end_of_file);
}

#[test]
fn req_w3_52_missing_begin_marker_returns_invalid_patch_error() {
    let err = parse_patch("not a patch").expect_err("missing begin marker should fail");
    match err {
        ParseError::InvalidPatchError(msg) => {
            assert!(msg.contains("Begin Patch"), "msg={msg}");
        }
        other => panic!("expected InvalidPatchError, got {other:?}"),
    }
}

#[test]
fn req_w3_52_invalid_hunk_header_returns_invalid_hunk_error() {
    let patch = "*** Begin Patch\n\
                 *** Bogus Hunk: foo\n\
                 *** End Patch";
    let err = parse_patch(patch).expect_err("bogus hunk header should fail");
    match err {
        ParseError::InvalidHunkError {
            message,
            line_number,
        } => {
            assert!(message.contains("not a valid hunk header"), "msg={message}");
            assert_eq!(line_number, 2);
        }
        other => panic!("expected InvalidHunkError, got {other:?}"),
    }
}

#[test]
fn req_w3_52_lenient_mode_strips_heredoc_wrapper() {
    let inner = "*** Begin Patch\n\
                 *** Add File: foo.txt\n\
                 +data\n\
                 *** End Patch";
    let patch = format!("<<'EOF'\n{inner}\nEOF\n");
    let args = parse_patch(&patch).expect("lenient mode should strip heredoc");
    assert_eq!(args.hunks.len(), 1);
}

#[test]
fn req_w3_52_empty_patch_body_yields_no_hunks() {
    let patch = "*** Begin Patch\n*** End Patch";
    let args = parse_patch(patch).expect("empty patch should parse");
    assert!(args.hunks.is_empty());
    assert_eq!(args.workdir, None);
}
