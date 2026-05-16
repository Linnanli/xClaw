//! Phase 3.2.E SECURITY pin **S21** — xargs target decomposition.
//!
//! Integration tests at the [`is_command_read_only`] boundary that prove
//! upstream `claude-code-main/src/tools/BashTool/readOnlyValidation.ts`
//! L1218–L1232 (`SAFE_TARGET_COMMANDS_FOR_XARGS`) + L1376–L1377
//! (dispatcher injection) + `readOnlyCommandValidation.ts` L1681–L1745
//! (validator's xargs-target branch) compose end-to-end:
//!
//! * Targets in the safe set (`echo` / `printf` / `wc` / `grep` /
//!   `head` / `tail`) are allowed under reasonable flag shapes.
//! * Any unsafe target (`ls` / `rm` / `mkdir` / `sh` / `bash` / `cat`),
//!   shell operator (`<` / `>` / `|`), unknown flag (`-J`), or
//!   non-literal `-I` argument flips the decision to `false`. This
//!   pins S21 (`xargs -I{} rm {} < list`).
//!
//! Sub-issue: <https://github.com/Linnanli/xClaw/issues/603> (Phase
//! 3.2.E.rest Part 1).
//!
//! Test matrix follows pairwise(2-way) coverage over:
//! - flag    ∈ {none, -I {}, -I X, -L 1, -n 1, -n1, -J X, -r}
//! - target  ∈ {echo, printf, wc, grep, head, tail, ls, rm, mkdir, sh}
//! - redirect∈ {none, < list, > out, | pipe}
//!
//! Cases below intentionally enumerate the boundary rows rather than
//! the full Cartesian product — each row corresponds to a documented
//! validator branch.

use dasclaw_bash_validation::is_command_read_only;

// ---------------------------------------------------------------------------
// Safe targets: each entry in SAFE_TARGET_COMMANDS_FOR_XARGS must allow
// the simplest "xargs <target>" form.
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_echo_allowed() {
    assert!(is_command_read_only("xargs echo"));
}

#[test]
fn req_security_s21_xargs_printf_allowed() {
    assert!(is_command_read_only("xargs printf"));
}

#[test]
fn req_security_s21_xargs_wc_allowed() {
    assert!(is_command_read_only("xargs wc"));
}

#[test]
fn req_security_s21_xargs_grep_allowed() {
    assert!(is_command_read_only("xargs grep"));
}

#[test]
fn req_security_s21_xargs_head_allowed() {
    assert!(is_command_read_only("xargs head"));
}

#[test]
fn req_security_s21_xargs_tail_allowed() {
    assert!(is_command_read_only("xargs tail"));
}

// ---------------------------------------------------------------------------
// Unsafe targets: NOT in SAFE_TARGET_COMMANDS_FOR_XARGS → reject.
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_ls_blocked() {
    assert!(!is_command_read_only("xargs ls"));
}

#[test]
fn req_security_s21_xargs_rm_blocked() {
    assert!(!is_command_read_only("xargs rm"));
}

#[test]
fn req_security_s21_xargs_mkdir_blocked() {
    assert!(!is_command_read_only("xargs mkdir"));
}

#[test]
fn req_security_s21_xargs_sh_blocked() {
    assert!(!is_command_read_only("xargs sh"));
}

#[test]
fn req_security_s21_xargs_bash_blocked() {
    assert!(!is_command_read_only("xargs bash"));
}

#[test]
fn req_security_s21_xargs_cat_blocked() {
    // `cat` is read-only but reads arbitrary files — NOT in the xargs
    // safe-target allowlist (UNC-on-Windows attack surface, upstream
    // L1218 SECURITY comment). xargs cannot dispatch to cat.
    assert!(!is_command_read_only("xargs cat"));
}

// ---------------------------------------------------------------------------
// -I flag interactions (Brace arg type).
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_dash_i_literal_brace_safe_target_allowed() {
    assert!(is_command_read_only("xargs -I {} echo"));
}

#[test]
fn req_security_s21_xargs_dash_i_literal_brace_unsafe_target_blocked() {
    // Canonical S21: `xargs -I{} rm {} < list` simplified to the
    // single-subcommand shape (compound `< list` is rejected
    // separately by the operator check below).
    assert!(!is_command_read_only("xargs -I {} rm"));
}

#[test]
fn req_security_s21_xargs_dash_i_non_literal_brace_blocked() {
    // -I expects literal `{}`; any other string is rejected by the
    // Brace flag-arg validator (upstream `validateFlagArgument`).
    assert!(!is_command_read_only("xargs -I X echo"));
}

// ---------------------------------------------------------------------------
// Other flag shapes.
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_dash_l_number_safe_target_allowed() {
    assert!(is_command_read_only("xargs -L 1 echo"));
}

#[test]
fn req_security_s21_xargs_dash_n_number_safe_target_allowed() {
    assert!(is_command_read_only("xargs -n 1 echo"));
}

#[test]
fn req_security_s21_xargs_dash_r_safe_target_allowed() {
    assert!(is_command_read_only("xargs -r echo"));
}

#[test]
fn req_security_s21_xargs_unknown_flag_blocked() {
    // -J is NOT in XARGS_FLAGS — unknown flag → reject regardless
    // of target safety.
    assert!(!is_command_read_only("xargs -J X echo"));
}

#[test]
fn req_security_s21_xargs_combined_arg_taking_bundle_blocked() {
    // `-rI` bundles `-r` (no-arg) with `-I` (arg-taking). Bundled
    // arg-taking flags create a parser differential with GNU getopt
    // — the validator rejects the whole bundle (existing flag_parser
    // unit test `req_bash_validation_320_a_rejects_combined_short_with_arg_taking_member`).
    assert!(!is_command_read_only("xargs -rI echo rm evil"));
}

// ---------------------------------------------------------------------------
// Operator interactions at THIS boundary.
//
// `is_command_read_only` consumes the flag-parsing dispatcher which
// tokenises with `shell_words` (no operator model). Operator bypass is
// guarded at a HIGHER layer:
//
// * `crate::validate_read_only` (legacy ValidationResult path used by
//   `BashValidationHook`) explicitly checks WRITE_REDIRECTIONS.
// * `command_writes_to_git_internal_paths` uses tree-sitter and catches
//   redirections into `HEAD` / `objects/` / `refs/` / `hooks/`.
//
// The cases below cover only what THIS function can decide: the canonical
// S21 attack with `< list` is rejected because `rm` is not a safe xargs
// target (the `<` is incidental). Pipe-bypass is rejected because the
// first token of the compound is `ls`, not `xargs` — the dispatcher only
// inspects the leading subcommand. Output-redirect-to-non-git is
// intentionally NOT pinned here; that path lives at the hook layer and
// is exercised by hook-level snapshot tests.
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_input_redirect_to_unsafe_target_blocked() {
    // Canonical S21: `< list` is incidental; the decisive rejection is
    // `rm` not being in SAFE_TARGET_COMMANDS_FOR_XARGS.
    assert!(!is_command_read_only("xargs -I {} rm < list"));
}

#[test]
fn req_security_s21_pipe_to_xargs_with_unsafe_target_blocked() {
    // `ls | xargs rm` — first token is `ls`, not `xargs`; the
    // dispatcher only inspects the leading subcommand, so the
    // xargs-decomposition path never fires. The whole compound is
    // rejected because `ls | xargs rm` does not match any single
    // allowlist prefix.
    assert!(!is_command_read_only("ls | xargs rm"));
}

// ---------------------------------------------------------------------------
// Expansion / substitution: defense in depth via
// contains_unquoted_expansion.
// ---------------------------------------------------------------------------

#[test]
fn req_security_s21_xargs_dollar_expansion_blocked() {
    assert!(!is_command_read_only("xargs echo $HOME"));
}

#[test]
fn req_security_s21_xargs_glob_expansion_blocked() {
    assert!(!is_command_read_only("xargs echo *"));
}
