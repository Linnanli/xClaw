//! End-to-end test for the `claw_code_interop` example (Issue #911, B5).
//!
//! Includes the example's module via `#[path]` so we exercise the exact
//! same code the binary runs, without having to copy the demo into
//! `src/`. The grep anchor test enforces that the published interop
//! doc lists the two helper crates by name, so future drift on either
//! side fails fast.

#[path = "../examples/claw_code_interop.rs"]
mod demo;

use demo::{AuditOutcome, run_demo};

#[tokio::test]
async fn req_dasclaw_cli_w5_911_claw_code_interop_e2e() {
    let workspace = tempfile::tempdir().expect("create tempdir");
    let outcome = run_demo(workspace.path())
        .await
        .expect("demo runs to completion");

    assert_eq!(outcome.final_text, "done", "final assistant text");
    assert_eq!(
        outcome.hello_txt, "world\n",
        "hello.txt must reflect the applied patch"
    );

    // Timeline must contain at least: block(bash rm -rf /), ok(bash ls),
    // ok(apply_patch hello.txt). Extra Allow-side bash gate entries are
    // not recorded (RecordingGate only logs Block) so the exact length
    // is 3.
    assert_eq!(
        outcome.audit_log.len(),
        3,
        "expected 3 audit entries, got {:#?}",
        outcome.audit_log
    );

    // [0] gate blocks `rm -rf /`
    let first = &outcome.audit_log[0];
    assert_eq!(first.tool, "bash");
    match &first.outcome {
        AuditOutcome::BlockedByGate(reason) => {
            assert!(
                !reason.is_empty(),
                "BlockedByGate must carry a non-empty reason"
            );
        }
        other => panic!("entry 0 expected BlockedByGate, got {other:?}"),
    }

    // [1] executor handled `ls`
    let second = &outcome.audit_log[1];
    assert_eq!(second.tool, "bash");
    match &second.outcome {
        AuditOutcome::ExecutorOk(text) => {
            assert!(
                text.contains("ls"),
                "ExecutorOk text should echo the bash command, got {text:?}"
            );
        }
        other => panic!("entry 1 expected ExecutorOk, got {other:?}"),
    }

    // [2] executor applied the patch
    let third = &outcome.audit_log[2];
    assert_eq!(third.tool, "apply_patch");
    match &third.outcome {
        AuditOutcome::ExecutorOk(text) => {
            assert!(
                text.contains("applied patch"),
                "ExecutorOk text should mention the patch summary, got {text:?}"
            );
        }
        other => panic!("entry 2 expected ExecutorOk, got {other:?}"),
    }
}

/// Documentation anchor: the published interop doc must continue to
/// name the two helper crates the demo wires up, so future renames /
/// removals fail this test before they reach review.
#[test]
fn req_dasclaw_cli_w5_911_claw_code_dasclaw_grep_anchor() {
    let doc_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/INTEROP_DASCLAW.md");
    let doc = std::fs::read_to_string(doc_path).unwrap_or_else(|e| panic!("read {doc_path}: {e}"));
    assert!(
        doc.contains("dasclaw_bash_validation"),
        "INTEROP_DASCLAW.md must reference dasclaw_bash_validation"
    );
    assert!(
        doc.contains("dasclaw_apply_patch"),
        "INTEROP_DASCLAW.md must reference dasclaw_apply_patch"
    );
}
