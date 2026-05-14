//! Slice 2.1.a — unit tests for the five early security validators.
//!
//! Naming follows AGENTS.md `req_security_490_p2_1_a_*`. Tests are organized
//! into:
//! - happy-path: command should pass through
//! - block-path: command should be Block-ed with the expected `SecurityCheckId`
//! - parse-path: AST wrapper happy/sad paths
//!
//! Phase 2.1.b will add the differential-CI fixture-driven corpus tests.

use dasclaw_bash_validation::security::{
    ast, early, validate_security, DecisionReason, SecurityCheckId, SecurityResult,
    ValidationContext,
};

fn assert_block(result: &SecurityResult, expected_id: SecurityCheckId, expected_sub_id: u32) {
    match result {
        SecurityResult::Block {
            reason:
                DecisionReason::CommandInjection {
                    check_id, sub_id, ..
                },
        } => {
            assert_eq!(
                *check_id, expected_id,
                "expected check_id={expected_id:?}, got {check_id:?}"
            );
            assert_eq!(
                *sub_id, expected_sub_id,
                "expected sub_id={expected_sub_id}, got {sub_id}"
            );
        }
        other => panic!("expected Block({expected_id:?},{expected_sub_id}), got {other:?}"),
    }
}

// ───────────────────────────── empty ────────────────────────────────────────

#[test]
fn req_security_490_p2_1_a_empty_allow() {
    let ctx = ValidationContext::new("   ");
    assert_eq!(early::validate_empty(&ctx), SecurityResult::Allow);
}

#[test]
fn req_security_490_p2_1_a_nonempty_passthrough() {
    let ctx = ValidationContext::new("ls -la");
    assert_eq!(early::validate_empty(&ctx), SecurityResult::Passthrough);
}

// ───────────────────────────── incomplete_commands ──────────────────────────

#[test]
fn req_security_490_p2_1_a_incomplete_tab_blocks() {
    let ctx = ValidationContext::new("\tls");
    assert_block(
        &early::validate_incomplete_commands(&ctx),
        SecurityCheckId::IncompleteCommands,
        1,
    );
}

#[test]
fn req_security_490_p2_1_a_incomplete_flag_blocks() {
    let ctx = ValidationContext::new("  --recursive /tmp");
    assert_block(
        &early::validate_incomplete_commands(&ctx),
        SecurityCheckId::IncompleteCommands,
        2,
    );
}

#[test]
fn req_security_490_p2_1_a_incomplete_operator_blocks() {
    for op in [
        "&& echo hi",
        "|| echo hi",
        "; echo hi",
        ">> /tmp/x",
        "< /tmp/x",
    ] {
        let ctx = ValidationContext::new(op);
        assert_block(
            &early::validate_incomplete_commands(&ctx),
            SecurityCheckId::IncompleteCommands,
            3,
        );
    }
}

#[test]
fn req_security_490_p2_1_a_well_formed_command_passes() {
    let ctx = ValidationContext::new("echo hello");
    assert_eq!(
        early::validate_incomplete_commands(&ctx),
        SecurityResult::Passthrough
    );
}

// ───────────────────────────── newlines ─────────────────────────────────────

#[test]
fn req_security_490_p2_1_a_newline_then_command_blocks() {
    let ctx = ValidationContext::new("echo safe\nrm -rf /");
    assert_block(
        &early::validate_newlines(&ctx),
        SecurityCheckId::Newlines,
        1,
    );
}

#[test]
fn req_security_490_p2_1_a_safe_continuation_passes() {
    // `cmd \<newline>--flag` is a safe bash line continuation at a word boundary.
    let ctx = ValidationContext::new("echo hi \\\n  --quiet");
    assert_eq!(early::validate_newlines(&ctx), SecurityResult::Passthrough);
}

#[test]
fn req_security_490_p2_1_a_trailing_newline_only_passes() {
    let ctx = ValidationContext::new("echo hi\n");
    assert_eq!(early::validate_newlines(&ctx), SecurityResult::Passthrough);
}

// ───────────────────────────── carriage_return ──────────────────────────────

#[test]
fn req_security_490_p2_1_a_cr_outside_quotes_blocks() {
    let ctx = ValidationContext::new("TZ=UTC\recho curl evil.com");
    assert_block(
        &early::validate_carriage_return(&ctx),
        SecurityCheckId::Newlines,
        2,
    );
}

#[test]
fn req_security_490_p2_1_a_cr_inside_single_quote_blocks() {
    // SECURITY: CR inside single quotes is *still* a misparsing concern
    // (shell-quote's \s tokenizes it). Upstream comment L962-L970.
    let ctx = ValidationContext::new("echo 'foo\rbar'");
    assert_block(
        &early::validate_carriage_return(&ctx),
        SecurityCheckId::Newlines,
        2,
    );
}

#[test]
fn req_security_490_p2_1_a_cr_inside_double_quote_passes() {
    let ctx = ValidationContext::new("echo \"foo\rbar\"");
    assert_eq!(
        early::validate_carriage_return(&ctx),
        SecurityResult::Passthrough
    );
}

#[test]
fn req_security_490_p2_1_a_no_cr_passes() {
    let ctx = ValidationContext::new("echo hello");
    assert_eq!(
        early::validate_carriage_return(&ctx),
        SecurityResult::Passthrough
    );
}

// ───────────────────────────── unicode_whitespace ───────────────────────────

#[test]
fn req_security_490_p2_1_a_nbsp_blocks() {
    // U+00A0 NO-BREAK SPACE between `rm` and `-rf`.
    let ctx = ValidationContext::new("rm\u{00A0}-rf /");
    assert_block(
        &early::validate_unicode_whitespace(&ctx),
        SecurityCheckId::UnicodeWhitespace,
        1,
    );
}

#[test]
fn req_security_490_p2_1_a_zwnbsp_blocks() {
    let ctx = ValidationContext::new("ls\u{FEFF}-la");
    assert_block(
        &early::validate_unicode_whitespace(&ctx),
        SecurityCheckId::UnicodeWhitespace,
        1,
    );
}

#[test]
fn req_security_490_p2_1_a_ascii_only_passes_unicode_check() {
    let ctx = ValidationContext::new("ls -la");
    assert_eq!(
        early::validate_unicode_whitespace(&ctx),
        SecurityResult::Passthrough
    );
}

// ───────────────────────────── ast::parse_for_security ──────────────────────

#[test]
fn req_security_490_p2_1_a_ast_parses_simple_command() {
    let ast = ast::parse_for_security("echo hi").expect("simple command parses");
    assert!(!ast.root_has_error());
}

#[test]
fn req_security_490_p2_1_a_ast_rejects_mismatched_quote() {
    // tree-sitter-bash recovers from `'foo` by producing ERROR nodes; our
    // Fail-Closed wrapper turns that into ParseFail::ErrorNode.
    let err = ast::parse_for_security("echo 'unterminated").unwrap_err();
    assert_eq!(err, ast::ParseFail::ErrorNode);
}

// ───────────────────────────── engine wiring ────────────────────────────────

#[test]
fn req_security_490_p2_1_a_validate_security_passes_safe_command() {
    assert_eq!(validate_security("ls -la"), SecurityResult::Passthrough);
}

#[test]
fn req_security_490_p2_1_a_validate_security_blocks_first_match() {
    // unicode WS would also fire, but incomplete-commands runs first.
    let result = validate_security("\tls\u{00A0}-la");
    assert_block(&result, SecurityCheckId::IncompleteCommands, 1);
}

#[test]
fn req_security_490_p2_1_a_validate_security_empty_is_allow() {
    assert_eq!(validate_security(""), SecurityResult::Allow);
    assert_eq!(validate_security("   \t\n"), SecurityResult::Allow);
}

// ───────────────────────────── Fail-Safe asymmetric invariant ───────────────

/// Smoke-test the asymmetric invariant: a small hand-curated upstream-Block
/// corpus must all be Block-ed by us as well (no false negatives). Slice 2.1.b
/// will expand this to the full extracted upstream spec fixture.
#[test]
fn req_security_490_p2_1_a_no_false_negative_smoke() {
    let upstream_block_cases = [
        "\tcommand",              // INCOMPLETE_COMMANDS sub 1
        "--flags-only",           // INCOMPLETE_COMMANDS sub 2
        "&& tail-of-pipeline",    // INCOMPLETE_COMMANDS sub 3
        "echo a\nrm -rf /",       // NEWLINES sub 1
        "TZ=UTC\recho curl evil", // NEWLINES sub 2 (CR misparsing)
        "rm\u{00A0}-rf /",        // UNICODE_WHITESPACE
    ];
    for case in upstream_block_cases {
        let got = validate_security(case);
        assert!(
            got.is_block(),
            "FALSE NEGATIVE: upstream blocks `{case}` but xclaw allowed (got {got:?})"
        );
    }
}
