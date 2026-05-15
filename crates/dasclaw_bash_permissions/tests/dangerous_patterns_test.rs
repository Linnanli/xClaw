//! Slice 2.2.g — `DANGEROUS_BASH_PATTERNS` + `is_dangerous_bash_allow_rule`
//! verbatim port from
//! `claude-code-main/src/utils/permissions/dangerousPatterns.ts` (L1-L80)
//! and `permissionSetup.ts` (L94-L147).

use dasclaw_bash_permissions::{
    is_dangerous_bash_allow_rule, is_dangerous_bash_allow_rule_value, CROSS_PLATFORM_CODE_EXEC,
};
use dasclaw_bash_permissions::{PermissionRuleValue, BASH_TOOL_NAME};

// ---------- non-Bash tool gate ----------

#[test]
fn req_perm_490_p2_2_g_01_non_bash_tool_never_dangerous() {
    // Upstream L95-L97. Only Bash rules are checked.
    assert!(!is_dangerous_bash_allow_rule("Read", Some("python:*")));
    assert!(!is_dangerous_bash_allow_rule("Edit", Some("*")));
    assert!(!is_dangerous_bash_allow_rule("WebFetch", None));
}

// ---------- tool-level allow ----------

#[test]
fn req_perm_490_p2_2_g_02_tool_level_allow_no_content() {
    // Upstream L100-L102. `Bash` with no rule content = allow ALL.
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, None));
}

#[test]
fn req_perm_490_p2_2_g_03_tool_level_allow_empty_content() {
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("")));
}

#[test]
fn req_perm_490_p2_2_g_04_tool_level_allow_whitespace_content() {
    // Defensive — `Bash("   ")` is trim-equivalent to no content.
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("   ")));
}

// ---------- standalone wildcard ----------

#[test]
fn req_perm_490_p2_2_g_05_standalone_wildcard() {
    // Upstream L109-L111.
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("*")));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("  *  ")));
}

// ---------- per-pattern match shapes ----------

#[test]
fn req_perm_490_p2_2_g_06_exact_pattern_python() {
    // Upstream L120-L122. Exact pattern is dangerous (it's a rule
    // saying "any python execution is allowed").
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("python")));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python3")
    ));
}

#[test]
fn req_perm_490_p2_2_g_07_colon_star_pattern() {
    // Upstream L125-L127.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python:*")
    ));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("node:*")));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("bash:*")));
}

#[test]
fn req_perm_490_p2_2_g_08_trailing_star() {
    // Upstream L130-L132. `python*` matches python, python3, etc.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python*")
    ));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("npx*")));
}

#[test]
fn req_perm_490_p2_2_g_09_space_star() {
    // Upstream L135-L137. `python *` matches `python script.py`.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python *")
    ));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("node *")));
}

#[test]
fn req_perm_490_p2_2_g_10_dash_flag_star() {
    // Upstream L140-L142. `python -c*` matches `python -c 'evil'`.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python -c*")
    ));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python -m *")
    ));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("node -e*")
    ));
}

// ---------- SECURITY: ANT_ONLY entries MUST NOT leak ----------

#[test]
fn req_perm_490_p2_2_g_11_ant_only_entries_not_ported() {
    // PIN — Issue #490 epic explicitly forbids porting ANT-only
    // patterns. None of these may be treated as dangerous bash allow
    // rules in x-claw (external user). If any one returns true, the
    // ANT-only segment has leaked into the cross-environment set —
    // **revert immediately**.
    let ant_only_excluded = [
        "fa run",
        "fa run:*",
        "fa run*",
        "coo",
        "coo:*",
        "coo*",
        "gh",
        "gh:*",
        "gh*",
        "gh api",
        "gh api:*",
        "gh api*",
        "curl",
        "curl:*",
        "curl*",
        "wget",
        "wget:*",
        "wget*",
        "git",
        "git:*",
        "git*",
        "kubectl",
        "kubectl:*",
        "kubectl*",
        "aws",
        "aws:*",
        "aws*",
        "gcloud",
        "gcloud:*",
        "gcloud*",
        "gsutil",
        "gsutil:*",
        "gsutil*",
    ];
    for content in ant_only_excluded {
        assert!(
            !is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some(content)),
            "ANT-only pattern leaked: {} (Issue #490 red line — must stay false for external users)",
            content
        );
    }
}

// ---------- per-pattern coverage ----------

#[test]
fn req_perm_490_p2_2_g_12_every_cross_platform_pattern_dangerous() {
    // Every entry in CROSS_PLATFORM_CODE_EXEC must trigger `:*` dangerous.
    for pat in CROSS_PLATFORM_CODE_EXEC {
        let s = format!("{}:*", pat);
        assert!(
            is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some(&s)),
            "expected dangerous for {}:*",
            pat
        );
    }
}

#[test]
fn req_perm_490_p2_2_g_13_extra_patterns_dangerous() {
    // The non-cross-platform extras (zsh, fish, eval, exec, env, xargs,
    // sudo) must also trigger dangerous.
    let extras = ["zsh", "fish", "eval", "exec", "env", "xargs", "sudo"];
    for pat in extras {
        let s = format!("{}:*", pat);
        assert!(
            is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some(&s)),
            "expected dangerous for {}:*",
            pat
        );
    }
}

// ---------- non-dangerous ----------

#[test]
fn req_perm_490_p2_2_g_14_specific_command_safe() {
    // Narrow rules — fine. `python script.py` is exact, not a prefix.
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python script.py")
    ));
    // `git status` not in cross-platform list (git is ANT-only) — safe
    // for external users.
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("git status")
    ));
    // `npm test` is exact, not `npm run:*`.
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("npm test")
    ));
}

#[test]
fn req_perm_490_p2_2_g_15_dash_pattern_without_trailing_star() {
    // PIN — `python -c 'print(1)'` is a SPECIFIC rule, not a wildcard.
    // Upstream L140 requires BOTH `starts_with("pattern -")` AND
    // `ends_with('*')`. Missing trailing star → not dangerous.
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("python -c 'print(1)'")
    ));
}

#[test]
fn req_perm_490_p2_2_g_16_substring_not_matching() {
    // `pyranha` contains `py` but doesn't match any pattern.
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("pyranha:*")
    ));
    // `mybash:*` shouldn't match `bash:*` (different name).
    assert!(!is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("mybash:*")
    ));
}

// ---------- case insensitivity ----------

#[test]
fn req_perm_490_p2_2_g_17_case_insensitive() {
    // Upstream L106 lowercases the content.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("PYTHON:*")
    ));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("Python *")
    ));
    assert!(is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("BASH")));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("Npm Run:*")
    ));
}

#[test]
fn req_perm_490_p2_2_g_18_trim_whitespace() {
    // Upstream L106 trims first.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("   python:*   ")
    ));
}

// ---------- multi-word patterns ----------

#[test]
fn req_perm_490_p2_2_g_19_multi_word_pattern_npm_run() {
    // `npm run` is a multi-word entry — same shape rules apply.
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("npm run")
    ));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("npm run:*")
    ));
    assert!(is_dangerous_bash_allow_rule(
        BASH_TOOL_NAME,
        Some("npm run *")
    ));
    // But `npm` alone is NOT in the list (only `npm run` is) — exact
    // `npm` not dangerous. Sanity-pins the matcher boundary.
    assert!(!is_dangerous_bash_allow_rule(BASH_TOOL_NAME, Some("npm")));
}

// ---------- convenience wrapper ----------

#[test]
fn req_perm_490_p2_2_g_20_value_wrapper_delegates() {
    let v = PermissionRuleValue {
        tool_name: BASH_TOOL_NAME.to_string(),
        rule_content: Some("python:*".to_string()),
    };
    assert!(is_dangerous_bash_allow_rule_value(&v));

    let safe = PermissionRuleValue {
        tool_name: BASH_TOOL_NAME.to_string(),
        rule_content: Some("git status".to_string()),
    };
    assert!(!is_dangerous_bash_allow_rule_value(&safe));

    let other_tool = PermissionRuleValue {
        tool_name: "Edit".to_string(),
        rule_content: Some("*".to_string()),
    };
    assert!(!is_dangerous_bash_allow_rule_value(&other_tool));
}
