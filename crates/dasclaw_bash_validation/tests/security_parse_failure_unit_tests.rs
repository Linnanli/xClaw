//! Slice 2.1.e — engine-level Fail-Closed catch-all
//! (`DecisionReason::ParseFailure`).
//!
//! Verifies that `ast::validate_parse_failure`, wired as the final pipeline
//! entry, refuses commands whose syntax tree-sitter cannot agree on,
//! closing the gap where unterminated quotes / missing braces previously
//! fell through every validator and reached the executor.
//!
//! Naming: `req_security_490_p2_1_e_parsefail_<scenario>`.

use dasclaw_bash_validation::security::{
    ast, validate_security, DecisionReason, SecurityCheckId, SecurityResult, ValidationContext,
};

fn assert_parse_failure(result: &SecurityResult) {
    match result {
        SecurityResult::Block {
            reason: DecisionReason::ParseFailure { message },
        } => {
            assert!(
                message.contains("Command syntax could not be safely parsed"),
                "unexpected parse-failure message: {message:?}"
            );
        }
        other => panic!("expected Block(ParseFailure), got {other:?}"),
    }
}

#[test]
fn req_security_490_p2_1_e_parsefail_unterminated_double_quote_validator() {
    let ctx = ValidationContext::new("echo \"unterminated");
    assert!(
        ctx.ast().is_err(),
        "precondition: AST must refuse unterminated DQ"
    );
    assert_parse_failure(&ast::validate_parse_failure(&ctx));
}

#[test]
fn req_security_490_p2_1_e_parsefail_valid_command_passes_validator() {
    let ctx = ValidationContext::new("ls -la /tmp");
    assert!(
        ctx.ast().is_ok(),
        "precondition: simple ls must parse cleanly"
    );
    assert!(matches!(
        ast::validate_parse_failure(&ctx),
        SecurityResult::Passthrough
    ));
}

#[test]
fn req_security_490_p2_1_e_parsefail_unterminated_dq_blocks_via_pipeline() {
    // No other validator catches a bare unterminated DQ → must surface
    // as ParseFailure (Fail-Closed), not Allow / Passthrough.
    let result = validate_security("echo \"unterminated");
    assert_parse_failure(&result);
}

#[test]
fn req_security_490_p2_1_e_parsefail_unterminated_sq_blocks_via_pipeline() {
    let result = validate_security("cat 'still open");
    assert_parse_failure(&result);
}

#[test]
fn req_security_490_p2_1_e_parsefail_unclosed_brace_group_blocks() {
    // `{ ls -la` is syntactically incomplete (open brace group never
    // closed) — tree-sitter rejects it and no earlier regex/AST validator
    // fires, so the Fail-Closed tail must surface ParseFailure.
    let result = validate_security("{ ls -la");
    assert_parse_failure(&result);
}

#[test]
fn req_security_490_p2_1_e_parsefail_safe_command_does_not_emit_parsefailure() {
    // Smoke test: a syntactically valid, semantically safe command must
    // not be misclassified as a parse failure.
    let result = validate_security("ls -la /tmp");
    assert!(
        !matches!(
            result,
            SecurityResult::Block {
                reason: DecisionReason::ParseFailure { .. }
            }
        ),
        "safe command must not surface ParseFailure: {result:?}"
    );
}

#[test]
fn req_security_490_p2_1_e_parsefail_concrete_validator_wins_over_parsefailure() {
    // `ls $(echo hi` has both an unclosed `$(` (tree-sitter ERROR) AND a
    // `$()` command-substitution pattern that the misparsing-sensitive
    // `validate_dangerous_patterns` matches. The concrete validator runs
    // earlier in the pipeline and is misparsing-sensitive, so it short-
    // circuits there; the Fail-Closed tail (placed last) never runs.
    //
    // Precondition check: confirm the AST still refuses parsing, so this
    // really would have hit ParseFailure absent the earlier validator.
    let ctx = ValidationContext::new("ls $(echo hi");
    assert!(ctx.ast().is_err(), "precondition: input must fail to parse");
    match validate_security("ls $(echo hi") {
        SecurityResult::Block {
            reason:
                DecisionReason::CommandInjection {
                    check_id: SecurityCheckId::DangerousPatternsCommandSubstitution,
                    ..
                },
        } => {}
        SecurityResult::Block {
            reason: DecisionReason::ParseFailure { .. },
        } => panic!("ParseFailure must not override an earlier misparsing-sensitive validator"),
        other => panic!("expected DangerousPatternsCommandSubstitution Block, got {other:?}"),
    }
}

#[test]
fn req_security_490_p2_1_e_parsefail_overrides_deferred_non_misparsing() {
    // `validate_newlines` is non-misparsing — its result is *deferred* and
    // only surfaces if no later misparsing-sensitive validator fires.
    // A command that (a) has a bare LF followed by content (triggers
    // newlines deferred) AND (b) has an unterminated DQ (triggers
    // ParseFailure) must surface ParseFailure, because Fail-Closed AST
    // refusal trumps a regex-only deferred verdict.
    //
    // Input: "echo a\nls \"unterm" — LF between two commands + unterminated DQ.
    let result = validate_security("echo a\nls \"unterm");
    // Build the equivalent ctx to assert the precondition cleanly.
    let ctx = ValidationContext::new("echo a\nls \"unterm");
    assert!(ctx.ast().is_err(), "precondition: input must fail to parse");
    assert_parse_failure(&result);
}

#[test]
fn req_security_490_p2_1_e_parsefail_empty_command_still_allowed() {
    // `validate_empty` runs early in the pipeline and returns Allow.
    // ParseFailure must not interfere with the Allow short-circuit.
    let result = validate_security("");
    assert!(
        matches!(result, SecurityResult::Allow),
        "empty input must remain Allow, not ParseFailure: {result:?}"
    );
}
