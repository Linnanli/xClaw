//! Slice 2.1.c2 — unit tests for the 7 AST / state-machine main validators
//! plus the `validate_git_commit` early validator.
//!
//! Naming: `req_security_490_p2_1_c2_<area>_<scenario>`.

use dasclaw_bash_validation::security::{
    ast_validators as a, early, validate_security, DecisionReason, SecurityCheckId, SecurityResult,
    ValidationContext,
};

fn ctx(command: &str) -> ValidationContext<'_> {
    ValidationContext::new(command)
}

fn assert_block(result: &SecurityResult, expected_id: SecurityCheckId, expected_sub_id: u32) {
    match result {
        SecurityResult::Block {
            reason:
                DecisionReason::CommandInjection {
                    check_id, sub_id, ..
                },
        } => {
            assert_eq!(*check_id, expected_id, "check_id mismatch");
            assert_eq!(*sub_id, expected_sub_id, "sub_id mismatch");
        }
        other => panic!("expected Block({expected_id:?},{expected_sub_id}), got {other:?}"),
    }
}

fn assert_pass(result: &SecurityResult) {
    assert!(
        matches!(result, SecurityResult::Passthrough),
        "expected Passthrough, got {result:?}"
    );
}

fn assert_allow(result: &SecurityResult) {
    assert!(
        matches!(result, SecurityResult::Allow),
        "expected Allow, got {result:?}"
    );
}

// ───────────────────────── validate_jq_command ──────────────────────────────

#[test]
fn req_security_490_p2_1_c2_jq_system_function_blocks() {
    let r = a::validate_jq_command(&ctx(r#"jq '.foo | system("rm -rf /")' file.json"#));
    assert_block(&r, SecurityCheckId::JqSystemFunction, 1);
}

#[test]
fn req_security_490_p2_1_c2_jq_from_file_flag_blocks() {
    let r = a::validate_jq_command(&ctx("jq -f script.jq data.json"));
    assert_block(&r, SecurityCheckId::JqFileArguments, 1);
}

#[test]
fn req_security_490_p2_1_c2_jq_rawfile_flag_blocks() {
    let r = a::validate_jq_command(&ctx("jq --rawfile key /etc/passwd ."));
    assert_block(&r, SecurityCheckId::JqFileArguments, 1);
}

#[test]
fn req_security_490_p2_1_c2_jq_library_path_blocks() {
    let r = a::validate_jq_command(&ctx("jq -L /tmp/evil '.foo'"));
    assert_block(&r, SecurityCheckId::JqFileArguments, 1);
}

#[test]
fn req_security_490_p2_1_c2_jq_safe_filter_passes() {
    let r = a::validate_jq_command(&ctx("jq '.users[] | .name' users.json"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_jq_non_jq_command_passes() {
    let r = a::validate_jq_command(&ctx("cat file.json | grep foo"));
    assert_pass(&r);
}

// ───────────────────────── validate_obfuscated_flags ────────────────────────

#[test]
fn req_security_490_p2_1_c2_obfuscated_ansi_c_quoting_blocks() {
    let r = a::validate_obfuscated_flags(&ctx(r"rm $'\x2d'rf /"));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 5);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_locale_quoting_blocks() {
    let r = a::validate_obfuscated_flags(&ctx(r#"rm $"-rf" /tmp"#));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 6);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_empty_quote_dash_blocks() {
    let r = a::validate_obfuscated_flags(&ctx("rm '' -rf /"));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 7);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_triple_quote_blocks() {
    // Triple quote at word start without trailing dash → hits sub_id 11.
    let r = a::validate_obfuscated_flags(&ctx("rm '''abc /tmp"));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 11);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_quoted_dash_in_flag_blocks() {
    let r = a::validate_obfuscated_flags(&ctx(r#"rm "-rf" /tmp"#));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 4);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_dash_with_quote_inside_blocks() {
    let r = a::validate_obfuscated_flags(&ctx(r#"rm -"r"f /tmp"#));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 1);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_plain_rm_passes() {
    let r = a::validate_obfuscated_flags(&ctx("rm -rf /tmp/foo"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_echo_with_quote_exempt() {
    // Simple echo without operators is exempt.
    let r = a::validate_obfuscated_flags(&ctx(r#"echo "-flag""#));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_obfuscated_cut_dash_d_quote_exempt() {
    // `cut -d` followed by quote is special-cased.
    let r = a::validate_obfuscated_flags(&ctx(r#"cut -d':' -f1 file"#));
    assert_pass(&r);
}

// ───────────────────── validate_backslash_escaped_whitespace ────────────────

#[test]
fn req_security_490_p2_1_c2_bs_whitespace_blocks() {
    let r = a::validate_backslash_escaped_whitespace(&ctx(r"rm\ -rf /tmp"));
    assert_block(&r, SecurityCheckId::BackslashEscapedWhitespace, 1);
}

#[test]
fn req_security_490_p2_1_c2_bs_whitespace_inside_single_quotes_passes() {
    let r = a::validate_backslash_escaped_whitespace(&ctx(r"echo 'hello\ world'"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_bs_whitespace_no_escape_passes() {
    let r = a::validate_backslash_escaped_whitespace(&ctx("rm -rf /tmp/foo"));
    assert_pass(&r);
}

// ───────────────────── validate_backslash_escaped_operators ─────────────────

#[test]
fn req_security_490_p2_1_c2_bs_operator_semicolon_blocks() {
    let r = a::validate_backslash_escaped_operators(&ctx(r"echo foo\;rm -rf /"));
    assert_block(&r, SecurityCheckId::BackslashEscapedOperators, 1);
}

#[test]
fn req_security_490_p2_1_c2_bs_operator_pipe_blocks() {
    let r = a::validate_backslash_escaped_operators(&ctx(r"echo foo\|cat /etc/passwd"));
    assert_block(&r, SecurityCheckId::BackslashEscapedOperators, 1);
}

#[test]
fn req_security_490_p2_1_c2_bs_operator_redirection_blocks() {
    let r = a::validate_backslash_escaped_operators(&ctx(r"echo foo\>out.txt"));
    assert_block(&r, SecurityCheckId::BackslashEscapedOperators, 1);
}

#[test]
fn req_security_490_p2_1_c2_bs_operator_inside_single_quotes_passes() {
    let r = a::validate_backslash_escaped_operators(&ctx(r"echo 'foo\;bar'"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_bs_operator_normal_passes() {
    let r = a::validate_backslash_escaped_operators(&ctx("echo foo; ls"));
    assert_pass(&r);
}

// ───────────────────────── validate_brace_expansion ─────────────────────────

#[test]
fn req_security_490_p2_1_c2_brace_comma_list_blocks() {
    let r = a::validate_brace_expansion(&ctx("echo {foo,bar}"));
    assert_block(&r, SecurityCheckId::BraceExpansion, 1);
}

#[test]
fn req_security_490_p2_1_c2_brace_range_blocks() {
    let r = a::validate_brace_expansion(&ctx("echo {1..10}"));
    assert_block(&r, SecurityCheckId::BraceExpansion, 1);
}

#[test]
fn req_security_490_p2_1_c2_brace_excess_close_blocks() {
    let r = a::validate_brace_expansion(&ctx("echo {foo}}"));
    assert_block(&r, SecurityCheckId::BraceExpansion, 2);
}

#[test]
fn req_security_490_p2_1_c2_brace_quoted_inside_unquoted_blocks() {
    let r = a::validate_brace_expansion(&ctx(r#"echo {a,"{",b}"#));
    assert_block(&r, SecurityCheckId::BraceExpansion, 3);
}

#[test]
fn req_security_490_p2_1_c2_brace_no_expansion_passes() {
    let r = a::validate_brace_expansion(&ctx("echo hello"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_brace_escaped_passes() {
    let r = a::validate_brace_expansion(&ctx(r"echo \{foo,bar\}"));
    assert_pass(&r);
}

// ──────────────────── validate_zsh_dangerous_commands ───────────────────────

#[test]
fn req_security_490_p2_1_c2_zsh_zmodload_blocks() {
    let r = a::validate_zsh_dangerous_commands(&ctx("zmodload zsh/system"));
    assert_block(&r, SecurityCheckId::ZshDangerousCommands, 1);
}

#[test]
fn req_security_490_p2_1_c2_zsh_with_precommand_modifier_blocks() {
    let r = a::validate_zsh_dangerous_commands(&ctx("noglob zmodload zsh/system"));
    assert_block(&r, SecurityCheckId::ZshDangerousCommands, 1);
}

#[test]
fn req_security_490_p2_1_c2_zsh_with_env_assignment_blocks() {
    let r = a::validate_zsh_dangerous_commands(&ctx("FOO=bar emulate sh"));
    assert_block(&r, SecurityCheckId::ZshDangerousCommands, 1);
}

#[test]
fn req_security_490_p2_1_c2_zsh_fc_dash_e_blocks() {
    let r = a::validate_zsh_dangerous_commands(&ctx("fc -e vim"));
    assert_block(&r, SecurityCheckId::ZshDangerousCommands, 2);
}

#[test]
fn req_security_490_p2_1_c2_zsh_normal_command_passes() {
    let r = a::validate_zsh_dangerous_commands(&ctx("ls -la"));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_zsh_fc_list_passes() {
    let r = a::validate_zsh_dangerous_commands(&ctx("fc -l"));
    assert_pass(&r);
}

// ──────────────── validate_malformed_token_injection ────────────────────────

#[test]
fn req_security_490_p2_1_c2_malformed_unbalanced_quote_with_separator_blocks() {
    let r = a::validate_malformed_token_injection(&ctx(r#"ls; echo "foo"#));
    assert_block(&r, SecurityCheckId::MalformedTokenInjection, 1);
}

#[test]
fn req_security_490_p2_1_c2_malformed_unbalanced_single_quote_blocks() {
    let r = a::validate_malformed_token_injection(&ctx(r"ls && echo 'foo"));
    assert_block(&r, SecurityCheckId::MalformedTokenInjection, 1);
}

#[test]
fn req_security_490_p2_1_c2_malformed_balanced_quote_passes() {
    let r = a::validate_malformed_token_injection(&ctx(r#"echo "foo"; ls"#));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_malformed_no_separator_passes() {
    let r = a::validate_malformed_token_injection(&ctx(r#"echo "unclosed"#));
    assert_pass(&r);
}

// ──────────────────────── validate_git_commit (early) ───────────────────────

#[test]
fn req_security_490_p2_1_c2_git_commit_safe_message_allows() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m "fix: typo""#));
    assert_allow(&r);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_dollar_paren_substitution_blocks() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m "release $(whoami)""#));
    assert_block(&r, SecurityCheckId::GitCommitSubstitution, 1);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_backtick_substitution_blocks() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m "release `whoami`""#));
    assert_block(&r, SecurityCheckId::GitCommitSubstitution, 1);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_brace_param_blocks() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m "release ${USER}""#));
    assert_block(&r, SecurityCheckId::GitCommitSubstitution, 1);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_single_quoted_substitution_safe() {
    // Single-quoted message: `$(...)` is not expanded → safe.
    let r = early::validate_git_commit(&ctx(r"git commit -m 'release $(whoami)'"));
    assert_allow(&r);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_message_starts_with_dash_blocks() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m "-rf""#));
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 5);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_with_backslash_falls_through() {
    let r = early::validate_git_commit(&ctx(r#"git commit -m \"foo\""#));
    assert_pass(&r);
}

#[test]
fn req_security_490_p2_1_c2_git_commit_non_git_passes() {
    let r = early::validate_git_commit(&ctx(r#"echo "git commit -m foo""#));
    assert_pass(&r);
}

// ───────────────────────── pipeline integration ─────────────────────────────

#[test]
fn req_security_490_p2_1_c2_pipeline_jq_system_blocks() {
    let r = validate_security(r#"jq 'system("id")' file.json"#);
    assert_block(&r, SecurityCheckId::JqSystemFunction, 1);
}

#[test]
fn req_security_490_p2_1_c2_pipeline_brace_expansion_blocks() {
    let r = validate_security("echo {foo,bar}");
    assert_block(&r, SecurityCheckId::BraceExpansion, 1);
}

#[test]
fn req_security_490_p2_1_c2_pipeline_zsh_blocks() {
    let r = validate_security("zmodload zsh/system");
    assert_block(&r, SecurityCheckId::ZshDangerousCommands, 1);
}

#[test]
fn req_security_490_p2_1_c2_pipeline_git_commit_safe_allows() {
    let r = validate_security(r#"git commit -m "fix: bug""#);
    assert_allow(&r);
}

#[test]
fn req_security_490_p2_1_c2_pipeline_bs_operator_blocks() {
    let r = validate_security(r"echo foo\;rm -rf /");
    assert_block(&r, SecurityCheckId::BackslashEscapedOperators, 1);
}

#[test]
fn req_security_490_p2_1_c2_pipeline_obfuscated_blocks() {
    let r = validate_security(r"rm $'\x2d'rf /");
    assert_block(&r, SecurityCheckId::ObfuscatedFlags, 5);
}
