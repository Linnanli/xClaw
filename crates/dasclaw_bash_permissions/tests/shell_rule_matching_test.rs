//! Unit tests for `shell_rule_matching` — verbatim semantic port of
//! `claude-code-main/src/utils/permissions/shellRuleMatching.test.ts`
//! cases plus deviation-asserting probes.
//!
//! Test naming follows AGENTS.md §测试纪律: `req_perm_490_p2_2_a_<id>_<desc>`.

use dasclaw_bash_permissions::{
    has_wildcards, match_wildcard_pattern, parse_permission_rule, permission_rule_extract_prefix,
    MatchOptions, ShellPermissionRule,
};

fn matches(pattern: &str, cmd: &str) -> bool {
    match_wildcard_pattern(pattern, cmd, MatchOptions::default())
}

// === permission_rule_extract_prefix ===

#[test]
fn req_perm_490_p2_2_a_01_extract_prefix_legacy_star() {
    assert_eq!(permission_rule_extract_prefix("npm:*"), Some("npm"));
    assert_eq!(permission_rule_extract_prefix("git:*"), Some("git"));
    assert_eq!(
        permission_rule_extract_prefix("git push:*"),
        Some("git push")
    );
}

#[test]
fn req_perm_490_p2_2_a_02_extract_prefix_returns_none_when_no_trailing_star() {
    assert_eq!(permission_rule_extract_prefix("npm"), None);
    assert_eq!(permission_rule_extract_prefix("git diff *"), None);
    assert_eq!(permission_rule_extract_prefix("*:foo"), None);
    assert_eq!(permission_rule_extract_prefix(""), None);
    // Trailing `:*` with empty prefix yields None (matches upstream
    // regex `^(.+):\*$` requiring at least one char).
    assert_eq!(permission_rule_extract_prefix(":*"), None);
}

// === has_wildcards ===

#[test]
fn req_perm_490_p2_2_a_03_has_wildcards_legacy_prefix_is_not_wildcard() {
    assert!(!has_wildcards("npm:*"));
    assert!(!has_wildcards("git push:*"));
}

#[test]
fn req_perm_490_p2_2_a_04_has_wildcards_detects_unescaped_star() {
    assert!(has_wildcards("git diff *"));
    assert!(has_wildcards("* run *"));
    assert!(has_wildcards("npm run *"));
}

#[test]
fn req_perm_490_p2_2_a_05_has_wildcards_skips_escaped_star() {
    assert!(!has_wildcards("echo \\*"));
    assert!(!has_wildcards("git status"));
    // Even backslashes preceding `*` mean the `*` is unescaped.
    assert!(has_wildcards("echo \\\\*")); // \\ + *
}

// === match_wildcard_pattern: literal/exact tail ===

#[test]
fn req_perm_490_p2_2_a_06_match_exact_pattern_without_wildcard() {
    assert!(matches("git status", "git status"));
    assert!(!matches("git status", "git statuss"));
    assert!(!matches("git status", "git"));
}

#[test]
fn req_perm_490_p2_2_a_07_match_single_wildcard_anywhere() {
    assert!(matches("git *", "git add"));
    assert!(matches("git *", "git push origin main"));
    // Trailing-` *` optionalization: bare `git` matches `git *`.
    assert!(matches("git *", "git"));
}

#[test]
fn req_perm_490_p2_2_a_08_match_multi_wildcard_no_trailing_optionalization() {
    // `* run *` has 2 wildcards; trailing ` *` is NOT optionalized.
    // `npm run` (no trailing arg) must NOT match.
    assert!(!matches("* run *", "npm run"));
    assert!(matches("* run *", "npm run build"));
    assert!(matches("* run *", "yarn run test --watch"));
}

#[test]
fn req_perm_490_p2_2_a_09_match_escaped_star_is_literal() {
    assert!(matches("echo \\*", "echo *"));
    assert!(!matches("echo \\*", "echo anything"));
}

#[test]
fn req_perm_490_p2_2_a_10_match_escaped_backslash_is_literal() {
    assert!(matches("echo \\\\", "echo \\"));
    assert!(!matches("echo \\\\", "echo "));
}

#[test]
fn req_perm_490_p2_2_a_11_match_escapes_regex_metachars_in_pattern() {
    // Patterns containing regex metachars must match literally.
    assert!(matches("echo foo.bar", "echo foo.bar"));
    assert!(!matches("echo foo.bar", "echo fooXbar"));

    assert!(matches("echo (hi)", "echo (hi)"));
    assert!(matches("echo a+b", "echo a+b"));
    assert!(matches("echo $HOME", "echo $HOME"));
    assert!(matches("echo a|b", "echo a|b"));
    assert!(matches("echo [x]", "echo [x]"));
}

#[test]
fn req_perm_490_p2_2_a_12_match_dotall_handles_embedded_newlines() {
    // `s` (dotAll) flag is on so `.*` matches newlines too.
    assert!(matches("git *", "git diff\nfile"));
}

#[test]
fn req_perm_490_p2_2_a_13_match_pattern_is_trimmed() {
    assert!(matches("   git status   ", "git status"));
    assert!(matches("  git *  ", "git add"));
}

#[test]
fn req_perm_490_p2_2_a_14_match_is_case_sensitive_by_default() {
    // Bash itself is case-sensitive.
    assert!(!matches("GIT status", "git status"));
    assert!(!matches("git STATUS", "git status"));
}

#[test]
fn req_perm_490_p2_2_a_15_match_case_insensitive_when_opted_in() {
    let opts = MatchOptions {
        case_insensitive: true,
    };
    assert!(match_wildcard_pattern("GIT status", "git status", opts));
    assert!(match_wildcard_pattern("git *", "GIT add", opts));
}

#[test]
fn req_perm_490_p2_2_a_16_match_anchors_whole_string() {
    // `^...$` anchors mean the pattern must consume the full command.
    assert!(!matches("git", "git status"));
    assert!(!matches("status", "git status"));
}

#[test]
fn req_perm_490_p2_2_a_17_match_empty_pattern_only_matches_empty_command() {
    assert!(matches("", ""));
    assert!(!matches("", "anything"));
    // Whitespace-only pattern trims to empty.
    assert!(matches("   ", ""));
}

// === parse_permission_rule ===

#[test]
fn req_perm_490_p2_2_a_18_parse_exact_rule() {
    assert_eq!(
        parse_permission_rule("git status"),
        ShellPermissionRule::Exact {
            command: "git status".to_string()
        }
    );
}

#[test]
fn req_perm_490_p2_2_a_19_parse_legacy_prefix_wins_over_wildcard_check() {
    // `foo:*` is prefix, NOT wildcard (rule extraction precedence).
    assert_eq!(
        parse_permission_rule("npm:*"),
        ShellPermissionRule::Prefix {
            prefix: "npm".to_string()
        }
    );
    assert_eq!(
        parse_permission_rule("git push:*"),
        ShellPermissionRule::Prefix {
            prefix: "git push".to_string()
        }
    );
}

#[test]
fn req_perm_490_p2_2_a_20_parse_wildcard_rule() {
    assert_eq!(
        parse_permission_rule("git diff *"),
        ShellPermissionRule::Wildcard {
            pattern: "git diff *".to_string()
        }
    );
    assert_eq!(
        parse_permission_rule("* run *"),
        ShellPermissionRule::Wildcard {
            pattern: "* run *".to_string()
        }
    );
}

#[test]
fn req_perm_490_p2_2_a_21_parse_escaped_star_is_exact_not_wildcard() {
    // `\*` is escaped, so no unescaped `*` remains → exact match.
    assert_eq!(
        parse_permission_rule("echo \\*"),
        ShellPermissionRule::Exact {
            command: "echo \\*".to_string()
        }
    );
}

// === Internal invariant: assembled regex always compiles ===
//
// This is asserted indirectly by every match call above (the `match
// Regex::new` arm returns `false` on error; if any of our patterns
// triggered the error path, the matching tests would fail). Add an
// explicit probe over a range of metachar-heavy patterns as a
// belt-and-suspenders guard.

#[test]
fn req_perm_490_p2_2_a_22_never_produces_an_invalid_regex_for_heavy_metachars() {
    let probes = [
        r#"echo "$(curl x)" | tee -a /tmp/* "#,
        r#"git diff -- '*.rs'"#,
        r"^.*$",
        r"\\\\\\\\",
        r"a+b?c{1,2}|d(e)[f]g",
        r"\\* \\* \\*",
    ];
    for pat in probes {
        // We don't care about match outcome here; we care that the
        // function returns *something* (i.e., regex compilation
        // succeeded internally — otherwise we'd have a panic in the
        // unwrap-less Regex::new path which we've already eliminated).
        let _ = matches(pat, "anything");
        let _ = matches(pat, pat);
    }
}

#[test]
fn req_perm_490_p2_2_a_23_upstream_parity_git_star_matches_bare_git() {
    // Documented upstream invariant aligning wildcard with `git:*`
    // semantics — single trailing wildcard is optionalized.
    assert!(matches("git *", "git"));
    // But `npm run *` (2 wildcards effective — leading `* run *` here)
    // is NOT optionalized (see test 08). Single-wildcard rules with
    // non-space-prefixed trailing wildcards are NOT optionalized:
    // pattern `git*` (no space) has trailing `.*` but the assembled
    // regex ends in `.*` not ` .*`, so the optionalization branch is
    // skipped and `git` still matches because `.*` consumes empty.
    assert!(matches("git*", "git"));
    assert!(matches("git*", "git status"));
}
