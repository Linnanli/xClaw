//! Tests for `suggestion` — Slice 2.2.n (Issue #490 Phase 2.2).
//!
//! Upstream parity matrix for `bashPermissions.ts::suggestionForExactCommand`
//! (L266-L295) + `suggestionForPrefix` (L339-L341) + helpers.
//!
//! Test ID: `req_perm_490_p2_2_n_NN_<desc>`.

use dasclaw_bash_permissions::{
    extract_prefix_before_heredoc, get_simple_command_prefix, suggestion_for_exact_command,
    suggestion_for_prefix, types::PermissionBehavior, BashRuleSuggestion, SuggestionDestination,
    BASH_TOOL_NAME,
};

fn rc(s: &BashRuleSuggestion) -> &str {
    s.rule_value.rule_content.as_deref().unwrap_or("")
}

// =================================================================
// get_simple_command_prefix
// =================================================================

// -----------------------------------------------------------------
// 01 — two-token form: `git status` → `git status`
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_01_two_token_prefix() {
    assert_eq!(
        get_simple_command_prefix("git status"),
        Some("git status".to_string())
    );
    assert_eq!(
        get_simple_command_prefix("npm run build"),
        Some("npm run".to_string())
    );
    assert_eq!(
        get_simple_command_prefix("docker compose up -d"),
        Some("docker compose".to_string())
    );
}

// -----------------------------------------------------------------
// 02 — single token returns None (no prefix to suggest)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_02_single_token_none() {
    assert_eq!(get_simple_command_prefix("whoami"), None);
    assert_eq!(get_simple_command_prefix("ls"), None);
}

// -----------------------------------------------------------------
// 03 — flag / filename / number rejects (subcommand shape gate)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_03_non_subcommand_second_token_rejects() {
    assert_eq!(get_simple_command_prefix("ls -la"), None);
    assert_eq!(get_simple_command_prefix("cat file.txt"), None);
    assert_eq!(get_simple_command_prefix("chmod 755 file"), None);
    assert_eq!(get_simple_command_prefix("cd /tmp"), None);
    // capital letter in second token also rejects
    assert_eq!(get_simple_command_prefix("git Status"), None);
}

// -----------------------------------------------------------------
// 04 — SAFE env-var preamble peeled, prefix taken from remainder
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_04_safe_env_var_skipped() {
    assert_eq!(
        get_simple_command_prefix("NODE_ENV=production npm run build"),
        Some("npm run".to_string())
    );
    assert_eq!(
        get_simple_command_prefix("TZ=UTC LANG=C git status"),
        Some("git status".to_string())
    );
}

// -----------------------------------------------------------------
// 05 — **SECURITY PIN**: unsafe env-var assignment returns None
//      (prevents suggesting rules that would never match)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_05_unsafe_env_var_returns_none() {
    // MALICIOUS_VAR not in SAFE_ENV_VARS — must return None so caller
    // falls back to exact-match suggestion.
    assert_eq!(get_simple_command_prefix("MALICIOUS=1 npm run build"), None);
    // PATH hijack attempt — also returns None
    assert_eq!(get_simple_command_prefix("PATH=/evil git status"), None);
    // LD_PRELOAD hijack — returns None
    assert_eq!(get_simple_command_prefix("LD_PRELOAD=/x.so ls"), None);
}

// =================================================================
// suggestion_for_prefix
// =================================================================

// -----------------------------------------------------------------
// 06 — normal prefix produces `<prefix>:*` allow / localSettings
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_06_normal_prefix_shape() {
    let out = suggestion_for_prefix("git status");
    assert_eq!(out.len(), 1);
    let s = &out[0];
    assert_eq!(s.rule_value.tool_name, BASH_TOOL_NAME);
    assert_eq!(rc(s), "git status:*");
    assert_eq!(s.behavior, PermissionBehavior::Allow);
    assert_eq!(s.destination, SuggestionDestination::LocalSettings);
}

// -----------------------------------------------------------------
// 07 — **SECURITY PIN**: bare shell (bash/sh/zsh/…) → empty Vec
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_07_bare_shell_blocked() {
    for shell in [
        "bash",
        "sh",
        "zsh",
        "fish",
        "csh",
        "tcsh",
        "ksh",
        "dash",
        "cmd",
        "powershell",
        "pwsh",
    ] {
        assert!(
            suggestion_for_prefix(shell).is_empty(),
            "bare shell `{shell}` MUST NOT be suggested"
        );
        // Also blocked when followed by an arg (the prefix has
        // shell as the FIRST token).
        assert!(
            suggestion_for_prefix(&format!("{shell} -c")).is_empty(),
            "`{shell} -c` MUST NOT be suggested"
        );
    }
}

// -----------------------------------------------------------------
// 08 — **SECURITY PIN**: wrapper blocked (env, xargs, nice, stdbuf,
//      nohup, timeout, time)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_08_wrapper_blocked() {
    for wrapper in ["env", "xargs", "nice", "stdbuf", "nohup", "timeout", "time"] {
        assert!(
            suggestion_for_prefix(wrapper).is_empty(),
            "wrapper `{wrapper}` MUST NOT be suggested as prefix (would degenerate to Bash(*))"
        );
    }
}

// -----------------------------------------------------------------
// 09 — **SECURITY PIN**: privilege-escalation blocked
//      (sudo / doas / pkexec)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_09_priv_escalation_blocked() {
    for esc in ["sudo", "doas", "pkexec"] {
        assert!(
            suggestion_for_prefix(esc).is_empty(),
            "privilege escalator `{esc}` MUST NOT be suggested"
        );
        assert!(
            suggestion_for_prefix(&format!("{esc} apt install")).is_empty(),
            "`{esc} apt install` MUST NOT be suggested"
        );
    }
}

// -----------------------------------------------------------------
// 10 — empty / whitespace prefix → empty Vec
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_10_empty_prefix() {
    assert!(suggestion_for_prefix("").is_empty());
    assert!(suggestion_for_prefix("   ").is_empty());
}

// =================================================================
// suggestion_for_exact_command — decision tree
// =================================================================

// -----------------------------------------------------------------
// 11 — single-line with 2-token prefix → prefix rule (`<p>:*`)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_11_single_line_prefix_path() {
    let out = suggestion_for_exact_command("git status --short");
    assert_eq!(out.len(), 1);
    assert_eq!(rc(&out[0]), "git status:*");
}

// -----------------------------------------------------------------
// 12 — single-token command falls back to exact-match suggestion
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_12_single_token_exact_fallback() {
    let out = suggestion_for_exact_command("whoami");
    assert_eq!(out.len(), 1);
    assert_eq!(rc(&out[0]), "whoami");
    assert_eq!(out[0].behavior, PermissionBehavior::Allow);
}

// -----------------------------------------------------------------
// 13 — flag-suffix command (no prefix possible) → exact-match
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_13_flag_suffix_exact() {
    let out = suggestion_for_exact_command("ls -la");
    assert_eq!(out.len(), 1);
    assert_eq!(rc(&out[0]), "ls -la");
}

// -----------------------------------------------------------------
// 14 — heredoc command → prefix from text BEFORE the `<<`
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_14_heredoc_prefix() {
    let out = suggestion_for_exact_command("cat <<EOF\nhello\nEOF");
    assert_eq!(out.len(), 1);
    // `cat` alone has only 1 token before `<<`; fallback to
    // 2-token-take takes just `cat`.
    assert_eq!(rc(&out[0]), "cat:*");
}

// -----------------------------------------------------------------
// 15 — heredoc with simple-command prefix preferred
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_15_heredoc_two_token_prefix_preferred() {
    let out = suggestion_for_exact_command("git commit -m \"$(cat <<'EOF'\nmsg body\nEOF\n)\"");
    assert_eq!(out.len(), 1);
    assert_eq!(rc(&out[0]), "git commit:*");
}

// -----------------------------------------------------------------
// 16 — multi-line (no heredoc) → first-line prefix-rule
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_16_multiline_first_line() {
    let out = suggestion_for_exact_command("npm run build\necho done");
    assert_eq!(out.len(), 1);
    // Multiline path takes the first line VERBATIM (upstream
    // `suggestionForPrefix(firstLine)`), not through
    // `get_simple_command_prefix`. So prefix = full first line.
    assert_eq!(rc(&out[0]), "npm run build:*");
}

// -----------------------------------------------------------------
// 17 — **SECURITY PIN**: `bash -c "evil"` exact → exact-match
//      (NOT `bash:*` — that would allow arbitrary code)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_17_bash_dash_c_exact_not_prefix() {
    let out = suggestion_for_exact_command("bash -c \"evil command\"");
    // get_simple_command_prefix("bash -c \"evil command\"") returns
    // None (second token `-c` fails subcommand shape) → exact-match.
    assert_eq!(out.len(), 1);
    assert_eq!(rc(&out[0]), "bash -c \"evil command\"");
}

// -----------------------------------------------------------------
// 18 — **SECURITY PIN**: command starting with `bash something` where
//      the 2-token prefix WOULD be `bash something` is blocked by
//      BARE_SHELL_PREFIXES — falls back to exact match.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_18_bash_two_word_prefix_blocked() {
    // `bash something` would pass shape check; BARE_SHELL_PREFIXES
    // blocks suggestion_for_prefix → empty Vec returned. So caller
    // (suggestion_for_exact_command) emits zero suggestions for this
    // input — upstream parity (L188-L226 rationale).
    let out = suggestion_for_exact_command("bash something");
    assert!(
        out.is_empty(),
        "bare-bash prefix MUST yield zero suggestions: {out:?}"
    );
}

// -----------------------------------------------------------------
// 19 — Determinism: same input → identical output across calls
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_19_deterministic() {
    let a = suggestion_for_exact_command("git status");
    let b = suggestion_for_exact_command("git status");
    assert_eq!(a, b);
}

// -----------------------------------------------------------------
// 20 — **SECURITY PIN**: SAFE env-var + dangerous command — the env
//      strip lets `npm run` surface as prefix (legitimate), but
//      `sudo` underneath would still be blocked.
//      Demonstrates layered defense — strip → prefix → BARE block.
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_20_layered_defense_env_plus_bare() {
    // SAFE env (TZ) + sudo command — prefix candidate would be
    // `sudo apt`. BARE_SHELL_PREFIXES blocks sudo → empty.
    let out = suggestion_for_exact_command("TZ=UTC sudo apt install foo");
    assert!(
        out.is_empty(),
        "sudo prefix MUST be blocked even after safe-env strip"
    );
}

// =================================================================
// extract_prefix_before_heredoc — direct helper tests
// =================================================================

// -----------------------------------------------------------------
// 21 — no heredoc → None
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_21_no_heredoc_none() {
    assert_eq!(extract_prefix_before_heredoc("echo hello"), None);
    assert_eq!(extract_prefix_before_heredoc("ls -la"), None);
}

// -----------------------------------------------------------------
// 22 — heredoc at position 0 → None (would yield empty prefix)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_22_heredoc_at_start_none() {
    assert_eq!(extract_prefix_before_heredoc("<<EOF\nhi\nEOF"), None);
}

// -----------------------------------------------------------------
// 23 — unsafe env-var before heredoc → None (Fail-Safe)
// -----------------------------------------------------------------
#[test]
fn req_perm_490_p2_2_n_23_heredoc_unsafe_env_none() {
    assert_eq!(
        extract_prefix_before_heredoc("MALICIOUS=1 cat <<EOF\nhi\nEOF"),
        None
    );
}
