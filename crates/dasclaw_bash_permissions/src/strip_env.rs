//! Env-var prefix + safe-wrapper stripping — port of upstream
//! `stripAllLeadingEnvVars`, `stripSafeWrappers`, and the
//! `BINARY_HIJACK_VARS` Fail-Safe regex from
//! `claude-code-main/src/tools/BashTool/bashPermissions.ts`
//! (L378-L466 for `SAFE_ENV_VARS`, L524-L676 for `stripSafeWrappers`,
//! L703-L777 for `stripAllLeadingEnvVars` + `BINARY_HIJACK_VARS`).
//!
//! ## Scope (Slice 2.2.e)
//!
//! Three helpers, **library-only**:
//!
//! - [`is_binary_hijack_var`] — pin upstream blocklist regex
//!   `^(LD_|DYLD_|PATH$)` (L708). These env vars change which / how a
//!   binary is resolved; stripping them in front of a deny rule would
//!   let `PATH=/evil rm` bypass `Bash(rm:*) deny`. Used as the
//!   `blocklist` parameter for [`strip_all_leading_env_vars`].
//!
//! - [`strip_all_leading_env_vars`] — broader env-var stripper used by
//!   the **deny / ask** rule path. Strips `FOO=bar`-style prefixes
//!   regardless of whether the var name is in the safe-list. Stops as
//!   soon as a hijack var is encountered (Fail-Safe). The value pattern
//!   intentionally excludes shell metacharacters so adversarial values
//!   like `FOO=$(id)` are NOT stripped.
//!
//! - [`strip_safe_wrappers`] — narrower stripper used by the **allow**
//!   rule path. Phase 1: strip env vars only when the var name is in
//!   [`SAFE_ENV_VARS`]. Phase 2: strip transparent wrappers
//!   (`timeout`, `time`, `nice`, `nohup`, `stdbuf`) per upstream
//!   `SAFE_WRAPPER_PATTERNS` (L524-L668).
//!
//! ## What is NOT in this slice (deferred to follow-ups)
//!
//! - `ANT_ONLY_SAFE_ENV_VARS` USER_TYPE gating (upstream L447-L505) —
//!   not relevant to x-claw distribution; an external user is the
//!   default. Adding this requires propagating `user_type` through the
//!   context, which is a separate refactor.
//! - Multi-line `stripCommentLines` (L500-L522) — single-line commands
//!   only; multi-line script handling is a Phase 2.3 concern.
//! - Quoted-value forms in `strip_all_leading_env_vars` (single + double
//!   quotes, `FOO='x'y"z"` concatenation, L729-L744). The simpler
//!   unquoted-only form here covers the vast majority of real inputs;
//!   the quoted forms are tracked in a follow-up — adversarial inputs
//!   that try to hide behind quotes will simply not be stripped, which
//!   is the **Fail-Safe** direction (deny rule still matches because
//!   the prefix stays).
//! - Hook wiring — Slice 2.2.f.
//! - Output redirection stripping — handled by tree-sitter AST when
//!   available (Phase 2.1) plus a Phase 2.3 concern.
//!
//! ## Forward pointers
//!
//! - `req_perm_490_p2_2_c_19_env_var_wrapping_not_yet_stripped` and
//!   `req_perm_490_p2_2_d_20_env_var_wrapper_not_stripped_yet` pin the
//!   behavior of `check_prefix_match` / `check_compound_match`
//!   **without** this stripping enabled — those pins stay green
//!   intentionally; this slice adds the building blocks, Slice 2.2.f
//!   will integrate them at the hook layer and flip those forward
//!   pointers.

use regex::Regex;
use std::sync::OnceLock;

/// Upstream `SAFE_ENV_VARS` (L378-L437). Allow-rule path strips
/// these env vars. Externally observable list — adding a new var here
/// changes the rule-match surface, so each addition must come from
/// upstream parity.
pub const SAFE_ENV_VARS: &[&str] = &[
    // Go
    "GOEXPERIMENT",
    "GOOS",
    "GOARCH",
    "CGO_ENABLED",
    "GO111MODULE",
    // Rust
    "RUST_BACKTRACE",
    "RUST_LOG",
    // Node
    "NODE_ENV",
    // Python
    "PYTHONUNBUFFERED",
    "PYTHONDONTWRITEBYTECODE",
    // Pytest
    "PYTEST_DISABLE_PLUGIN_AUTOLOAD",
    "PYTEST_DEBUG",
    // API keys
    "ANTHROPIC_API_KEY",
    // Locale
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "LC_TIME",
    "CHARSET",
    // Terminal
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "FORCE_COLOR",
    "TZ",
    // Color
    "LS_COLORS",
    "LSCOLORS",
    "GREP_COLOR",
    "GREP_COLORS",
    "GCC_COLORS",
    // Display
    "TIME_STYLE",
    "BLOCK_SIZE",
    "BLOCKSIZE",
];

/// Upstream `BINARY_HIJACK_VARS = /^(LD_|DYLD_|PATH$)/` (L708).
///
/// Returns `true` when the env var name matches the blocklist — these
/// vars change *which binary actually runs* (dynamic linker preload,
/// `PATH` lookup), so stripping them in front of a deny rule lets
/// adversarial input bypass the rule. When this returns `true`,
/// [`strip_all_leading_env_vars`] stops stripping (Fail-Safe).
pub fn is_binary_hijack_var(name: &str) -> bool {
    name.starts_with("LD_") || name.starts_with("DYLD_") || name == "PATH"
}

// Pattern for the **broader** env-var form used by the deny-rule path.
// Mirrors upstream L729-L744 minus the quoted-value alternations
// (deferred — see module docs). Allows the safe unquoted value charset
// `[A-Za-z0-9_./:+@~,=-]`, which is intentionally wider than the
// allow-path so that values like `FOO=a=b` are stripped (preventing
// trivial bypass), while still excluding shell metacharacters that
// could lead to expansion (`$ ` ` ; | & ( ) < > \ ' " \n \r`).
fn deny_env_var_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        #[expect(clippy::expect_used)]
        Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*)\+?=[A-Za-z0-9_./:+@~,=\-]*[ \t]+"#)
            .expect("static deny env-var regex")
    })
}

// Pattern for the **narrower** env-var form used by the allow-rule path
// (`strip_safe_wrappers` Phase 1). Mirrors upstream L568-L573 exactly:
// `^([A-Za-z_][A-Za-z0-9_]*)=([A-Za-z0-9_./:-]+)[ \t]+`. Tighter value
// charset than the deny path to keep the allow-list-gated stripping
// conservative.
fn allow_env_var_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        #[expect(clippy::expect_used)]
        Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*)=([A-Za-z0-9_./:\-]+)[ \t]+"#)
            .expect("static allow env-var regex")
    })
}

/// Strip ALL leading env var prefixes for the deny / ask rule path.
///
/// Iterates the broader env-var pattern; if a stripped var name matches
/// [`is_binary_hijack_var`] the loop aborts and we return the
/// **non-stripped** suffix at that iteration (Fail-Safe — the deny rule
/// keeps the hijack-prefix visible so it still won't match).
///
/// Trailing trim is applied on the way out.
pub fn strip_all_leading_env_vars(command: &str) -> String {
    let mut s: &str = command;
    loop {
        let Some(caps) = deny_env_var_pattern().captures(s) else {
            break;
        };
        let var_name = &caps[1];
        if is_binary_hijack_var(var_name) {
            // Fail-Safe — leave the hijack prefix in place.
            break;
        }
        let full_match_end = caps.get(0).map(|m| m.end()).unwrap_or(0);
        if full_match_end == 0 {
            break;
        }
        s = &s[full_match_end..];
    }
    s.trim().to_string()
}

// Upstream `SAFE_WRAPPER_PATTERNS` (L524-L668). Ported as Rust regex
// strings; the entries are anchored to the start of the (already-
// env-stripped) command. KEEP THE COMMENT ANCHORS — each entry maps to
// the upstream line range, which is what makes drift-check tractable.
fn safe_wrapper_patterns() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        // SECURITY: each value charset MUST stay an allowlist, never `[^ \t]+`
        // — see upstream L532-L545 (`timeout -k$(id) 10 ls` bypass).
        let raw_patterns: &[&str] = &[
            // timeout (upstream L546-L555)
            r"^timeout[ \t]+(?:(?:--(?:foreground|preserve-status|verbose)|--(?:kill-after|signal)=[A-Za-z0-9_.+\-]+|--(?:kill-after|signal)[ \t]+[A-Za-z0-9_.+\-]+|-v|-[ks][ \t]+[A-Za-z0-9_.+\-]+|-[ks][A-Za-z0-9_.+\-]+)[ \t]+)*(?:--[ \t]+)?\d+(?:\.\d+)?[smhd]?[ \t]+",
            // time (upstream L556)
            r"^time[ \t]+(?:--[ \t]+)?",
            // nice (upstream L558-L567)
            r"^nice(?:[ \t]+-n[ \t]+-?\d+|[ \t]+-\d+)?[ \t]+(?:--[ \t]+)?",
            // stdbuf (upstream L569-L572)
            r"^stdbuf(?:[ \t]+-[ioe][LN0-9]+)+[ \t]+(?:--[ \t]+)?",
            // nohup (upstream L573)
            r"^nohup[ \t]+(?:--[ \t]+)?",
        ];
        raw_patterns
            .iter()
            .map(|p| {
                #[expect(clippy::expect_used)]
                {
                    Regex::new(p).expect("static safe-wrapper regex")
                }
            })
            .collect()
    })
}

/// Strip leading SAFE_ENV_VARS-only env vars followed by safe wrappers
/// (allow-rule path).
///
/// Two phases per upstream L580-L676:
/// - Phase 1: strip env vars whose name is in [`SAFE_ENV_VARS`]
/// - Phase 2: strip wrapper commands one at a time until fixed-point
///
/// Returns the trimmed remainder. If nothing matches, returns the
/// trimmed input unchanged.
pub fn strip_safe_wrappers(command: &str) -> String {
    let mut s: String = command.to_string();

    // Phase 1: SAFE_ENV_VARS-only env-var stripping.
    loop {
        let Some(caps) = allow_env_var_pattern().captures(&s) else {
            break;
        };
        let var_name = caps[1].to_string();
        if !SAFE_ENV_VARS.contains(&var_name.as_str()) {
            // Non-safe env var — stop stripping (allow-path is
            // intentionally conservative; see plan §3.2).
            break;
        }
        let full_match_end = caps.get(0).map(|m| m.end()).unwrap_or(0);
        if full_match_end == 0 {
            break;
        }
        s = s[full_match_end..].to_string();
    }

    // Phase 2: wrapper stripping (fixed-point).
    loop {
        let mut replaced = false;
        for re in safe_wrapper_patterns() {
            if let Some(m) = re.find(&s) {
                s = s[m.end()..].to_string();
                replaced = true;
            }
        }
        if !replaced {
            break;
        }
    }

    s.trim().to_string()
}
