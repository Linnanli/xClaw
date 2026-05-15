//! Slice 2.2.e — env-var prefix + safe-wrapper stripping tests.
//!
//! Verbatim port pins for upstream
//! `claude-code-main/src/tools/BashTool/bashPermissions.ts`:
//! - `BINARY_HIJACK_VARS = /^(LD_|DYLD_|PATH$)/` (L708)
//! - `stripAllLeadingEnvVars` deny-rule semantics (L703-L777)
//! - `SAFE_ENV_VARS` allow-list (L378-L437)
//! - `stripSafeWrappers` two-phase strip (L524-L676)
//! - `SAFE_WRAPPER_PATTERNS` time/timeout/nice/nohup/stdbuf (L545-L573)
//!
//! Forward-pointers explicitly preserved:
//! - 2.2.c test 19 + 2.2.d test 20: `check_prefix_match` /
//!   `check_compound_match` do NOT auto-invoke these strippers. The
//!   helpers shipped here are library-only opt-in; Slice 2.2.f wires
//!   them into the hook pipeline and flips those pins.

use dasclaw_bash_permissions::{
    is_binary_hijack_var, strip_all_leading_env_vars, strip_safe_wrappers, SAFE_ENV_VARS,
};

// ---------- is_binary_hijack_var ----------

#[test]
fn req_perm_490_p2_2_e_01_hijack_ld_preload() {
    // Upstream pattern: `^(LD_|DYLD_|PATH$)`. `LD_*` prefix matches.
    assert!(is_binary_hijack_var("LD_PRELOAD"));
    assert!(is_binary_hijack_var("LD_LIBRARY_PATH"));
}

#[test]
fn req_perm_490_p2_2_e_02_hijack_dyld() {
    // macOS dynamic linker prefix.
    assert!(is_binary_hijack_var("DYLD_INSERT_LIBRARIES"));
    assert!(is_binary_hijack_var("DYLD_LIBRARY_PATH"));
}

#[test]
fn req_perm_490_p2_2_e_03_hijack_path_exact_only() {
    // `PATH$` means exact match; PATHEXT and PATH_BACKUP are NOT hijack vars
    // (only `PATH` is). Pin: do not regress to a substring/prefix check.
    assert!(is_binary_hijack_var("PATH"));
    assert!(!is_binary_hijack_var("PATHEXT"));
    assert!(!is_binary_hijack_var("PATH_BACKUP"));
    assert!(!is_binary_hijack_var("MYPATH"));
}

#[test]
fn req_perm_490_p2_2_e_04_hijack_unrelated_safe() {
    assert!(!is_binary_hijack_var("TZ"));
    assert!(!is_binary_hijack_var("NODE_ENV"));
    assert!(!is_binary_hijack_var("FOO"));
    assert!(!is_binary_hijack_var(""));
}

// ---------- strip_all_leading_env_vars ----------

#[test]
fn req_perm_490_p2_2_e_05_deny_strip_simple_var() {
    assert_eq!(strip_all_leading_env_vars("FOO=bar cmd arg"), "cmd arg");
}

#[test]
fn req_perm_490_p2_2_e_06_deny_strip_multiple_vars() {
    assert_eq!(
        strip_all_leading_env_vars("FOO=bar BAZ=qux cmd arg"),
        "cmd arg"
    );
}

#[test]
fn req_perm_490_p2_2_e_07_deny_strip_safe_chars() {
    // Pin upstream value charset `[A-Za-z0-9_./:+@~,=\-]` — value with
    // `=` (e.g. `FOO=a=b`) is stripped; otherwise adversary could hide
    // by inserting `=` in the value.
    assert_eq!(strip_all_leading_env_vars("FOO=a=b cmd"), "cmd");
    assert_eq!(strip_all_leading_env_vars("CFG=/etc/conf.d/x cmd"), "cmd");
}

#[test]
fn req_perm_490_p2_2_e_08_deny_failsafe_on_ld_preload() {
    // PIN — Fail-Safe: when a BINARY_HIJACK_VAR is encountered, stop
    // stripping. The hijack prefix stays attached to the command so the
    // downstream deny rule still cannot match `cmd:*`.
    let s = "LD_PRELOAD=/tmp/evil.so cmd";
    assert_eq!(strip_all_leading_env_vars(s), s);
}

#[test]
fn req_perm_490_p2_2_e_09_deny_failsafe_on_path() {
    let s = "PATH=/tmp/evil:/usr/bin cmd";
    assert_eq!(strip_all_leading_env_vars(s), s);
}

#[test]
fn req_perm_490_p2_2_e_10_deny_failsafe_stops_at_hijack() {
    // Strip leading safe vars then stop when hijack is seen. The
    // **remaining suffix** including the hijack var stays.
    assert_eq!(
        strip_all_leading_env_vars("FOO=bar PATH=/x cmd"),
        "PATH=/x cmd"
    );
}

#[test]
fn req_perm_490_p2_2_e_11_deny_strip_pathext_ok() {
    // PATHEXT is NOT in BINARY_HIJACK_VARS — pure name, no privilege.
    assert_eq!(
        strip_all_leading_env_vars("PATHEXT=.exe;.bat cmd"),
        // semicolon is NOT in value charset, so value match ends at
        // `=.exe` and `;.bat cmd` stays. This is the conservative path
        // — Fail-Safe — adversarial values are not silently swallowed.
        "PATHEXT=.exe;.bat cmd"
    );
    // Plain alphanumeric value strips cleanly.
    assert_eq!(strip_all_leading_env_vars("PATHEXT=exe cmd"), "cmd");
}

#[test]
fn req_perm_490_p2_2_e_12_deny_reject_shell_metachar_values() {
    // SECURITY PIN — values containing `$`, backtick, `;`, `|`, `(`,
    // `&` MUST NOT match the env-var pattern. The whole command stays.
    // Adversarial: `FOO=$(id) cmd` — if we stripped, `cmd` would match
    // an allow rule while `$(id)` actually executes.
    let attacks = [
        "FOO=$(id) cmd",
        "FOO=`id` cmd",
        "FOO=a;ls cmd",
        "FOO=a|ls cmd",
        "FOO=a&ls cmd",
        "FOO=a>b cmd",
    ];
    for a in attacks {
        assert_eq!(strip_all_leading_env_vars(a), *a, "must not strip: {}", a);
    }
}

#[test]
fn req_perm_490_p2_2_e_13_deny_no_change_when_no_env() {
    assert_eq!(strip_all_leading_env_vars("git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_14_deny_trim_only() {
    assert_eq!(strip_all_leading_env_vars("  git status  "), "git status");
}

#[test]
fn req_perm_490_p2_2_e_15_deny_empty() {
    assert_eq!(strip_all_leading_env_vars(""), "");
    assert_eq!(strip_all_leading_env_vars("   "), "");
}

// ---------- strip_safe_wrappers ----------

#[test]
fn req_perm_490_p2_2_e_16_allow_strip_safe_env_only() {
    // Only env vars whose name is in SAFE_ENV_VARS are stripped.
    assert_eq!(strip_safe_wrappers("TZ=UTC date"), "date");
    assert_eq!(
        strip_safe_wrappers("NODE_ENV=production npm test"),
        "npm test"
    );
}

#[test]
fn req_perm_490_p2_2_e_17_allow_stops_at_unsafe_env() {
    // PIN — `DOCKER_HOST` is NOT in `SAFE_ENV_VARS` (it's ANT_ONLY,
    // intentionally deferred). Stop stripping. This prevents a Bash(docker
    // ps:*) rule from matching `DOCKER_HOST=tcp://evil docker ps`.
    let s = "DOCKER_HOST=tcp://evil docker ps";
    assert_eq!(strip_safe_wrappers(s), s);
}

#[test]
fn req_perm_490_p2_2_e_18_allow_strip_timeout() {
    assert_eq!(strip_safe_wrappers("timeout 10 git status"), "git status");
    assert_eq!(strip_safe_wrappers("timeout 5s git status"), "git status");
    assert_eq!(strip_safe_wrappers("timeout 10.5 git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_19_allow_strip_timeout_flags() {
    // GNU long flags, fused + space-separated.
    assert_eq!(
        strip_safe_wrappers("timeout --signal=TERM 10 git status"),
        "git status"
    );
    assert_eq!(
        strip_safe_wrappers("timeout -k 5 10 git status"),
        "git status"
    );
    assert_eq!(
        strip_safe_wrappers("timeout --kill-after=5 10 git status"),
        "git status"
    );
}

#[test]
fn req_perm_490_p2_2_e_20_allow_reject_timeout_dangerous_flag_value() {
    // SECURITY PIN — `timeout -k$(id) 10 ls` upstream L539-L545. The
    // value charset is `[A-Za-z0-9_.+\-]` allowlist, NOT `[^ \t]+`. The
    // whole `timeout -k$(id) 10 ls` must NOT strip, otherwise `$(id)`
    // expands at exec time while we matched `Bash(ls:*)`.
    let s = "timeout -k$(id) 10 ls";
    assert_eq!(strip_safe_wrappers(s), s);
}

#[test]
fn req_perm_490_p2_2_e_21_allow_strip_time() {
    assert_eq!(strip_safe_wrappers("time git status"), "git status");
    assert_eq!(strip_safe_wrappers("time -- git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_22_allow_strip_nice() {
    // Three forms per upstream L567 (bare, -n N, -N).
    assert_eq!(strip_safe_wrappers("nice git status"), "git status");
    assert_eq!(strip_safe_wrappers("nice -n 10 git status"), "git status");
    assert_eq!(strip_safe_wrappers("nice -10 git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_23_allow_strip_nohup() {
    assert_eq!(strip_safe_wrappers("nohup git status"), "git status");
    assert_eq!(strip_safe_wrappers("nohup -- git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_24_allow_strip_stdbuf() {
    assert_eq!(strip_safe_wrappers("stdbuf -o0 git status"), "git status");
    assert_eq!(
        strip_safe_wrappers("stdbuf -o0 -eL git status"),
        "git status"
    );
}

#[test]
fn req_perm_490_p2_2_e_25_allow_chained_env_then_wrapper() {
    // Phase 1 strips TZ=UTC, Phase 2 strips timeout 10 → final `git status`.
    assert_eq!(
        strip_safe_wrappers("TZ=UTC timeout 10 git status"),
        "git status"
    );
}

#[test]
fn req_perm_490_p2_2_e_26_allow_no_match_passthrough() {
    assert_eq!(strip_safe_wrappers("git status"), "git status");
}

#[test]
fn req_perm_490_p2_2_e_27_allow_trim_empty() {
    assert_eq!(strip_safe_wrappers("  git status  "), "git status");
    assert_eq!(strip_safe_wrappers(""), "");
}

#[test]
fn req_perm_490_p2_2_e_28_safe_env_vars_list_sanity() {
    // Pin a representative subset of upstream SAFE_ENV_VARS (L378-L437)
    // — if this drifts, the allow-path stripping silently changes.
    let must_contain = [
        "GOOS",
        "RUST_BACKTRACE",
        "NODE_ENV",
        "ANTHROPIC_API_KEY",
        "LANG",
        "TERM",
        "TZ",
        "LS_COLORS",
        "TIME_STYLE",
    ];
    for v in must_contain {
        assert!(
            SAFE_ENV_VARS.contains(&v),
            "SAFE_ENV_VARS must contain {} (upstream parity)",
            v
        );
    }
    // Things that MUST NOT leak in.
    let must_not_contain = [
        "NODE_OPTIONS",
        "PYTHONPATH",
        "LD_PRELOAD",
        "PATH",
        "DOCKER_HOST",
        "KUBECONFIG",
    ];
    for v in must_not_contain {
        assert!(
            !SAFE_ENV_VARS.contains(&v),
            "SAFE_ENV_VARS MUST NOT contain {} (security pin)",
            v
        );
    }
}

#[test]
fn req_perm_490_p2_2_e_29_forward_pointer_check_prefix_unchanged() {
    // PIN — `check_prefix_match` is NOT yet wired to strip env vars
    // (Slice 2.2.c test 19 + 2.2.d test 20 pin this). This slice ships
    // helpers only — Slice 2.2.f integrates them at the hook layer.
    //
    // This forward-pointer test asserts the helpers are *callable* but
    // does NOT assert any change to the prefix matcher's behavior.
    let stripped = strip_all_leading_env_vars("LD_PRELOAD=/x rm -rf /");
    assert_eq!(stripped, "LD_PRELOAD=/x rm -rf /"); // Fail-Safe kept hijack prefix.
    let allow_stripped = strip_safe_wrappers("TZ=UTC timeout 5 rm -rf /");
    assert_eq!(allow_stripped, "rm -rf /");
}

#[test]
fn req_perm_490_p2_2_e_30_deny_append_form_strips() {
    // `FOO+=bar` is upstream-recognized as an env-var assignment
    // (L729+ `(\+?=)`). Our deny pattern allows the `+=` form so an
    // adversary can't bypass deny-rule matching with the append form.
    assert_eq!(strip_all_leading_env_vars("FOO+=bar cmd"), "cmd");
}
