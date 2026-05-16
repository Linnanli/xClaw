//! Phase 3.2.C.rest — readonly tests for `git_allowlist`, `gh_allowlist`,
//! and the `USER_TYPE=ant` + Windows-xargs-strip gates in
//! `bash_allowlist::get_command_allowlist()`.
//!
//! Each test name follows the AGENTS.md `req_<module>_<id>_<desc>` convention.
//! Mutating `USER_TYPE` is process-global; we serialize ant-gated cases via
//! a `Mutex` and always restore the prior value to avoid bleeding into other
//! tests that may read the same variable.

use std::sync::Mutex;

use dasclaw_bash_validation::readonly::bash_allowlist::{
    get_command_allowlist, is_command_safe_via_flag_parsing,
};

// ---------- env helpers ----------
static USER_TYPE_LOCK: Mutex<()> = Mutex::new(());

struct UserTypeGuard {
    prev: Option<String>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl UserTypeGuard {
    fn set(value: Option<&str>) -> Self {
        let lock = USER_TYPE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("USER_TYPE").ok();
        match value {
            Some(v) => unsafe { std::env::set_var("USER_TYPE", v) },
            None => unsafe { std::env::remove_var("USER_TYPE") },
        }
        Self { prev, _lock: lock }
    }
}

impl Drop for UserTypeGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var("USER_TYPE", v) },
            None => unsafe { std::env::remove_var("USER_TYPE") },
        }
    }
}

// ============================================================================
// GIT — pickaxe / opt-arg parser differential pins
// ============================================================================

#[test]
fn req_readonly_3_2_c_git_diff_pickaxe_consumes_output_flag() {
    // SECURITY S20: `-S` declares FlagArgType::String so the parser consumes
    // the NEXT token as its value. `--output=...` ends up as the consumed
    // value rather than being interpreted as a redirect flag — but because
    // the next-after token is missing, the validator rejects this anyway
    // (pickaxe needs a pattern). The key invariant: if a user tries to slip
    // `--output=FILE` past `-S`, it must not be treated as a top-level flag.
    let _g = UserTypeGuard::set(None);
    // `-S PAT --output=FILE` — pattern is consumed, --output=FILE then fails
    // because it's not in safe_flags. Either way: blocked.
    assert!(!is_command_safe_via_flag_parsing(
        "git diff -S PAT --output=/tmp/leak"
    ));
}

#[test]
fn req_readonly_3_2_c_git_diff_orderfile_string_flag_accepted() {
    let _g = UserTypeGuard::set(None);
    // `-O orderfile` is FlagArgType::String — accepted with an argument.
    assert!(is_command_safe_via_flag_parsing("git diff -O orderfile"));
}

#[test]
fn req_readonly_3_2_c_git_ls_remote_server_option_blocked() {
    // `--server-option` intentionally NOT in safe_flags.
    let _g = UserTypeGuard::set(None);
    assert!(!is_command_safe_via_flag_parsing(
        "git ls-remote --server-option=x origin"
    ));
}

#[test]
fn req_readonly_3_2_c_git_cat_file_batch_blocked() {
    // `--batch` intentionally excluded (can read arbitrary objects via stdin).
    let _g = UserTypeGuard::set(None);
    assert!(!is_command_safe_via_flag_parsing("git cat-file --batch"));
}

// ============================================================================
// GIT — callback-driven rejection
// ============================================================================

#[test]
fn req_readonly_3_2_c_git_remote_show_alphanumeric_only() {
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing("git remote show origin"));
    // Non-alphanumeric positional → rejected by git_remote_show_callback.
    // NB: use `.` (not `$`) so the bypass scan for shell metacharacters does
    // not short-circuit before the callback runs — we want to exercise the
    // callback's `/^[a-zA-Z0-9_-]+$/` regex, not the meta-character bypass.
    assert!(!is_command_safe_via_flag_parsing("git remote show foo.bar"));
    assert!(!is_command_safe_via_flag_parsing("git remote show ../etc"));
}

#[test]
fn req_readonly_3_2_c_git_remote_only_verbose_flag() {
    let _g = UserTypeGuard::set(None);
    // Bare `git remote` lists remotes.
    assert!(is_command_safe_via_flag_parsing("git remote"));
    // `-v` / `--verbose` accepted (only allowed flag for bare remote).
    assert!(is_command_safe_via_flag_parsing("git remote -v"));
    // Subcommand-style positional `add` blocked by git_remote_callback.
    assert!(!is_command_safe_via_flag_parsing(
        "git remote add origin https://x"
    ));
}

#[test]
fn req_readonly_3_2_c_git_tag_requires_list_flag() {
    let _g = UserTypeGuard::set(None);
    // Bare `git tag mytag` would CREATE a tag — must be blocked.
    assert!(!is_command_safe_via_flag_parsing("git tag mytag"));
    // `-l` / `--list` gate opens the listing form.
    assert!(is_command_safe_via_flag_parsing("git tag -l v1.*"));
    assert!(is_command_safe_via_flag_parsing("git tag --list"));
    // `--` semantic: positional appearing after `--` is still creation.
    assert!(!is_command_safe_via_flag_parsing("git tag -- mytag"));
}

#[test]
fn req_readonly_3_2_c_git_branch_requires_list_flag() {
    let _g = UserTypeGuard::set(None);
    // Bare `git branch newbranch` would CREATE a branch — blocked.
    assert!(!is_command_safe_via_flag_parsing("git branch newbranch"));
    // `--merged HEAD` is an optional-arg listing form — accepted.
    assert!(is_command_safe_via_flag_parsing("git branch --merged HEAD"));
    // `-l` / `--list` gate.
    assert!(is_command_safe_via_flag_parsing("git branch -l"));
}

#[test]
fn req_readonly_3_2_c_git_reflog_blocks_destructive_subcommands() {
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing("git reflog"));
    // `expire` / `delete` / `exists` mutate the reflog — blocked by callback.
    assert!(!is_command_safe_via_flag_parsing("git reflog expire HEAD"));
    assert!(!is_command_safe_via_flag_parsing(
        "git reflog delete HEAD@{0}"
    ));
    assert!(!is_command_safe_via_flag_parsing("git reflog exists HEAD"));
}

// ============================================================================
// GIT — longest-prefix order
// ============================================================================

#[test]
fn req_readonly_3_2_c_git_remote_show_precedes_git_remote() {
    // `git remote show` is matched as a multi-word entry, NOT as
    // `git remote` with a `show` positional (which the bare-remote callback
    // would reject). Verifies longest-prefix order in GIT_READ_ONLY_COMMANDS.
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing("git remote show origin"));
}

// ============================================================================
// GH — danger callback rejects URL/SSH/HOST/OWNER/REPO
// ============================================================================

#[test]
fn req_readonly_3_2_c_gh_callback_rejects_url_value() {
    let _g = UserTypeGuard::set(Some("ant"));
    // Positional URL.
    assert!(!is_command_safe_via_flag_parsing(
        "gh pr view https://evil.com/owner/repo/extra"
    ));
    // --repo=URL form (inline value).
    assert!(!is_command_safe_via_flag_parsing(
        "gh pr view --repo=https://evil.com/o/r"
    ));
}

#[test]
fn req_readonly_3_2_c_gh_callback_rejects_ssh_style() {
    let _g = UserTypeGuard::set(Some("ant"));
    assert!(!is_command_safe_via_flag_parsing(
        "gh pr view --repo=git@github.com:o/r"
    ));
}

#[test]
fn req_readonly_3_2_c_gh_callback_rejects_three_segment_host() {
    let _g = UserTypeGuard::set(Some("ant"));
    // HOST/OWNER/REPO has 2 slashes → 3 segments → rejected.
    assert!(!is_command_safe_via_flag_parsing(
        "gh pr view --repo evil.com/owner/repo"
    ));
}

#[test]
fn req_readonly_3_2_c_gh_callback_accepts_owner_repo() {
    let _g = UserTypeGuard::set(Some("ant"));
    // Normal OWNER/REPO has 1 slash → accepted.
    assert!(is_command_safe_via_flag_parsing(
        "gh pr view --repo owner/repo"
    ));
    assert!(is_command_safe_via_flag_parsing("gh pr list -R owner/repo"));
}

// ============================================================================
// USER_TYPE=ant gate
// ============================================================================

#[test]
fn req_readonly_3_2_c_gh_blocked_when_user_type_not_ant() {
    let _g = UserTypeGuard::set(None);
    // gh entries are NOT in the base allowlist.
    assert!(!is_command_safe_via_flag_parsing("gh pr list"));
    assert!(!is_command_safe_via_flag_parsing("gh issue view 1"));
    assert!(!is_command_safe_via_flag_parsing("aki -k foo"));
}

#[test]
fn req_readonly_3_2_c_gh_accepted_when_user_type_is_ant() {
    let _g = UserTypeGuard::set(Some("ant"));
    assert!(is_command_safe_via_flag_parsing("gh pr list"));
    assert!(is_command_safe_via_flag_parsing("gh auth status"));
    assert!(is_command_safe_via_flag_parsing("aki -k foo --json"));
}

#[test]
fn req_readonly_3_2_c_get_command_allowlist_ant_extends_base() {
    let _g_base = UserTypeGuard::set(None);
    let base_len = get_command_allowlist().len();
    drop(_g_base);

    let _g_ant = UserTypeGuard::set(Some("ant"));
    let ant_len = get_command_allowlist().len();
    // ANT adds 22 gh commands + aki = 23.
    assert_eq!(ant_len, base_len + 23);
}

// ============================================================================
// xargs windows strip (runtime cfg!() check)
// ============================================================================

#[cfg(not(target_os = "windows"))]
#[test]
fn req_readonly_3_2_c_xargs_present_on_non_windows() {
    let _g = UserTypeGuard::set(None);
    let names: Vec<&str> = get_command_allowlist().iter().map(|(n, _)| *n).collect();
    assert!(names.contains(&"xargs"));
}

#[cfg(target_os = "windows")]
#[test]
fn req_readonly_3_2_c_xargs_stripped_on_windows() {
    let _g = UserTypeGuard::set(None);
    let names: Vec<&str> = get_command_allowlist().iter().map(|(n, _)| *n).collect();
    assert!(!names.contains(&"xargs"));
}

// ============================================================================
// Existing baseline (smoke) — confirm 3.2.B + 3.2.C.2.a entries still match
// ============================================================================

#[test]
fn req_readonly_3_2_c_smoke_existing_entries_unaffected() {
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing("grep -ri pat src"));
    assert!(is_command_safe_via_flag_parsing("rg --type rust pat"));
    assert!(is_command_safe_via_flag_parsing("docker logs container"));
    assert!(is_command_safe_via_flag_parsing("pyright --outputjson"));
}

// ============================================================================
// Regression pins for Layer 1 audit fixes (verbatim-parity invariants)
// ============================================================================

#[test]
fn req_readonly_3_2_c_git_branch_format_excluded_security() {
    // Upstream readOnlyCommandValidation.ts L865 intentionally excludes
    // `--format` from `git branch` safeFlags. `--format=%(refname)` etc.
    // can be exploited; this PIN ensures we never re-introduce it.
    let _g = UserTypeGuard::set(None);
    assert!(!is_command_safe_via_flag_parsing(
        "git branch --format=%(refname) -l"
    ));
    assert!(!is_command_safe_via_flag_parsing(
        "git branch --format=oops"
    ));
}

#[test]
fn req_readonly_3_2_c_git_branch_merged_no_arg_allowed() {
    // Upstream L866-L867: --merged/--no-merged are 'none' (optional commit
    // arg handled in callback). Bare `git branch --merged` lists merged
    // branches; `git branch --merged HEAD` lists filtered.
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing("git branch --merged"));
    assert!(is_command_safe_via_flag_parsing("git branch --no-merged"));
    assert!(is_command_safe_via_flag_parsing("git branch --merged HEAD"));
}

#[test]
fn req_readonly_3_2_c_git_rev_parse_short_takes_string() {
    // Upstream L522: `--short: 'string'` (optional length via =N). Both
    // `--short` (no arg) and `--short=7` should be accepted, paired with
    // `--verify <sha>` for a typical use case.
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing(
        "git rev-parse --short HEAD"
    ));
    assert!(is_command_safe_via_flag_parsing(
        "git rev-parse --short=7 HEAD"
    ));
    assert!(is_command_safe_via_flag_parsing(
        "git rev-parse --verify HEAD"
    ));
}

#[test]
fn req_readonly_3_2_c_git_cat_file_batch_check_and_undetermined() {
    // Upstream L641: --batch-check is the safe stdin-batch variant.
    // Upstream L645: spelling is `--allow-undetermined-type`, NOT
    // `--allow-unknown-type` (real-git man page wording is misleading).
    let _g = UserTypeGuard::set(None);
    assert!(is_command_safe_via_flag_parsing(
        "git cat-file --batch-check"
    ));
    assert!(is_command_safe_via_flag_parsing(
        "git cat-file --allow-undetermined-type -p HEAD"
    ));
    // Misspelling — must NOT be accepted.
    assert!(!is_command_safe_via_flag_parsing(
        "git cat-file --allow-unknown-type -p HEAD"
    ));
    // --batch (without --check) is intentionally excluded.
    assert!(!is_command_safe_via_flag_parsing("git cat-file --batch"));
}

#[test]
fn req_readonly_3_2_c_git_tag_create_reflog_excluded() {
    // `--create-reflog` was a stray in the Rust port not present upstream.
    // Without --list it is interpreted as a flag-with-no-positional → still
    // safe (no creation), but `git tag --create-reflog foo` previously
    // mis-classified the bundle. After verbatim restore, --create-reflog is
    // unknown so the whole command is unsafe.
    let _g = UserTypeGuard::set(None);
    assert!(!is_command_safe_via_flag_parsing(
        "git tag --create-reflog -l v1*"
    ));
}

#[test]
fn req_readonly_3_2_c_git_for_each_ref_strays_excluded() {
    // Upstream L652-L672 has NO --color/--no-color/--shell/--perl/--python/
    // --tcl/--ignore-case. Regression pin for stray-flag drop.
    let _g = UserTypeGuard::set(None);
    assert!(!is_command_safe_via_flag_parsing(
        "git for-each-ref --color refs/heads"
    ));
    assert!(!is_command_safe_via_flag_parsing(
        "git for-each-ref --shell"
    ));
    // Known-safe flag still works.
    assert!(is_command_safe_via_flag_parsing(
        "git for-each-ref --format=%(refname) refs/heads"
    ));
}

#[test]
fn req_readonly_3_2_c_gh_respects_double_dash_true() {
    // Audit Fix 8: all gh entries default to respects_double_dash=true.
    // This means `gh pr view -- foo` ends flag parsing at `--`. Without a
    // dangerous URL/SSH/owner-repo positional, gh entries accept harmless
    // post-`--` tokens.
    let _g = UserTypeGuard::set(Some("ant"));
    assert!(is_command_safe_via_flag_parsing("gh pr view -- 123"));
    assert!(is_command_safe_via_flag_parsing("gh repo view"));
}
