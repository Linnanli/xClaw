//! `RuleSuggestion` generators — Slice 2.2.n (Issue #490 Phase 2.2).
//!
//! Semantic port of upstream `bashPermissions.ts` L266-L342 (bash-specific
//! decision logic) + `shellRuleMatching.ts` L189-L232 (shared shape
//! builders).
//!
//! Given a command that was Denied or Asked at the permission gate, these
//! generators produce the "always allow this" rule-suggestion buttons
//! that the desktop / CLI permission dialog offers users.
//!
//! ## Decision tree (matches upstream)
//!
//! 1. **Heredoc** (`<<` present): suggest prefix rule from text before the
//!    operator — exact-match would never match again because heredoc body
//!    changes per invocation. Helper:
//!    [`extract_prefix_before_heredoc`].
//! 2. **Multi-line** (no heredoc): use first non-empty line as a prefix
//!    rule (avoids `:*` characters appearing mid-pattern, which would
//!    corrupt the settings JSON).
//! 3. **Single-line**: if the command yields a "simple command prefix"
//!    (2 tokens like `git status` / `npm run`), suggest the prefix.
//!    Otherwise fall back to the exact command. Helper:
//!    [`get_simple_command_prefix`].
//!
//! ## SECURITY pins
//!
//! - **`BARE_SHELL_PREFIXES`**: `bash`, `sh`, `env`, `sudo`, `nice`,
//!   `timeout`, … are blocked from ever appearing as a prefix
//!   suggestion. Upstream rationale (L188-L226): `Bash(bash:*)` would
//!   allow arbitrary code via `-c "evil"`; `Bash(env:*)` would let
//!   `env bash -c "evil"` survive `strip_safe_wrappers`;
//!   `Bash(sudo:*)` is a privilege-escalation auto-approval.
//! - **`SAFE_ENV_VARS` gate**: env-var assignment tokens at the head are
//!   skipped only if their name appears in
//!   [`crate::strip_env::SAFE_ENV_VARS`]. If an unrecognised env var is
//!   found, the function returns `None` so the caller falls back to
//!   exact-match — this prevents generating prefix rules that can
//!   never match (e.g. `Bash(npm run:*)` would never match
//!   `MALICIOUS=1 npm run build` because `strip_safe_wrappers` won't
//!   peel `MALICIOUS=`).
//!
//! ## Deviation from upstream
//!
//! - `ANT_ONLY_SAFE_ENV_VARS` USER_TYPE branch SKIPPED (Issue #490 red
//!   line "明确禁止 port ANT-only").
//! - Returns local [`BashRuleSuggestion`] value type instead of upstream
//!   `PermissionUpdate[]` —- consumer (`dasclaw_hooks`) wraps into
//!   `x_claw_agent::RuleSuggestion` at the hook boundary.

use crate::exact_match::BASH_TOOL_NAME;
use crate::strip_env::SAFE_ENV_VARS;
use crate::types::{PermissionBehavior, PermissionRuleValue};
use regex::Regex;
use std::sync::OnceLock;

// -- Public types -----------------------------------------------------

/// Where a suggested rule, if accepted, should be persisted. Mirrors
/// upstream `PermissionUpdateDestination` (subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SuggestionDestination {
    /// Project-local settings (default for fresh suggestions —
    /// upstream `'localSettings'`).
    LocalSettings,
    /// Ephemeral, session-only.
    Session,
    /// User-global settings.
    UserSettings,
    /// Project settings (shared / committed).
    ProjectSettings,
}

/// One concrete rule suggestion produced for a Denied / Asked command.
///
/// Carries enough to round-trip through the desktop UI: the
/// fully-shaped [`PermissionRuleValue`], the proposed `behavior`
/// (typically [`PermissionBehavior::Allow`] for the "always allow"
/// button), and the suggested storage `destination`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BashRuleSuggestion {
    pub rule_value: PermissionRuleValue,
    pub behavior: PermissionBehavior,
    pub destination: SuggestionDestination,
}

// -- Constants & regexes ---------------------------------------------

/// Upstream `BARE_SHELL_PREFIXES` (bashPermissions.ts L196-L226).
/// **SECURITY PIN**: rules built from these tokens would either allow
/// arbitrary code (`bash -c`, `env bash -c`), or auto-escalate
/// (`sudo …`), or bypass the deny gate (`nice rm -rf /`). Never
/// suggested as a prefix.
const BARE_SHELL_PREFIXES: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "fish",
    "csh",
    "tcsh",
    "ksh",
    "dash",
    "cmd",
    "powershell",
    "pwsh",
    // wrappers that exec their args as a command
    "env",
    "xargs",
    // wrappers that `strip_safe_wrappers` peels — would degenerate to `Bash(*)`
    "nice",
    "stdbuf",
    "nohup",
    "timeout",
    "time",
    // privilege escalation
    "sudo",
    "doas",
    "pkexec",
];

fn env_var_assign_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // safety: literal pattern, validated at unit-test time below
    RE.get_or_init(|| Regex::new(r"^[A-Za-z_]\w*=").expect("BUG: ENV_VAR_ASSIGN_RE is literal"))
}

fn subcommand_shape_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$").expect("BUG: literal") // safety: literal regex
    })
}

// -- Helpers ---------------------------------------------------------

fn is_safe_env_var(name: &str) -> bool {
    SAFE_ENV_VARS.contains(&name)
}

/// Port of upstream `getSimpleCommandPrefix` (bashPermissions.ts L161-L188).
///
/// Returns a 2-token prefix (e.g. `"git status"`, `"npm run"`) if the
/// trimmed command starts with a safe env-var preamble followed by at
/// least 2 tokens, the second of which matches the subcommand shape
/// `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`. Returns `None` for flags,
/// filenames, numbers, paths, or unsafe env-var assignments.
///
/// ANT-only safe-env branch is **omitted** (Issue #490 red line).
pub fn get_simple_command_prefix(command: &str) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut i = 0;
    while i < tokens.len() && env_var_assign_re().is_match(tokens[i]) {
        let var_name = tokens[i].split('=').next().unwrap_or("");
        if !is_safe_env_var(var_name) {
            return None;
        }
        i += 1;
    }
    let remaining: &[&str] = &tokens[i..];
    if remaining.len() < 2 {
        return None;
    }
    let subcmd = remaining[1];
    if !subcommand_shape_re().is_match(subcmd) {
        return None;
    }
    Some(format!("{} {}", remaining[0], remaining[1]))
}

/// Port of upstream `extractPrefixBeforeHeredoc` (bashPermissions.ts L302-L335).
///
/// If `command` contains `<<` (heredoc operator), returns a stable
/// prefix taken from the text before it — preferring the 2-token form
/// from [`get_simple_command_prefix`], falling back to up to 2 tokens
/// after skipping SAFE env-var assignments. Returns `None` if the
/// command has no heredoc, or the prefix would be unsafe / unstable.
pub fn extract_prefix_before_heredoc(command: &str) -> Option<String> {
    let idx = command.find("<<")?;
    if idx == 0 {
        return None;
    }
    let before = command[..idx].trim();
    if before.is_empty() {
        return None;
    }
    if let Some(prefix) = get_simple_command_prefix(before) {
        return Some(prefix);
    }
    // Fallback: skip safe env-var assignments and take up to 2 tokens.
    let tokens: Vec<&str> = before.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() && env_var_assign_re().is_match(tokens[i]) {
        let var_name = tokens[i].split('=').next().unwrap_or("");
        if !is_safe_env_var(var_name) {
            return None;
        }
        i += 1;
    }
    if i >= tokens.len() {
        return None;
    }
    let take = tokens[i..].iter().take(2).copied().collect::<Vec<_>>();
    if take.is_empty() {
        None
    } else {
        Some(take.join(" "))
    }
}

fn is_bare_shell_prefix(prefix: &str) -> bool {
    // Upstream's BARE_SHELL_PREFIXES check is applied to the FIRST token
    // of a candidate prefix (e.g. `"bash -c"` → `bash` blocked).
    let first = prefix.split_whitespace().next().unwrap_or("");
    BARE_SHELL_PREFIXES.contains(&first)
}

// -- Public generators ----------------------------------------------

fn allow_local(rule_content: String) -> BashRuleSuggestion {
    BashRuleSuggestion {
        rule_value: PermissionRuleValue {
            tool_name: BASH_TOOL_NAME.to_string(),
            rule_content: Some(rule_content),
        },
        behavior: PermissionBehavior::Allow,
        destination: SuggestionDestination::LocalSettings,
    }
}

/// Generate suggestion(s) for an exact-match Denied / Asked command.
///
/// Port of `bashPermissions.ts::suggestionForExactCommand` (L266-L295).
///
/// Decision tree (in order):
///   1. heredoc present → prefix rule from prefix-before-heredoc
///   2. multi-line       → prefix rule from first non-empty line
///   3. simple-command-prefix present → prefix rule (`<prefix>:*`)
///   4. else             → exact-match rule (command verbatim)
pub fn suggestion_for_exact_command(command: &str) -> Vec<BashRuleSuggestion> {
    if let Some(prefix) = extract_prefix_before_heredoc(command) {
        return suggestion_for_prefix(&prefix);
    }
    if command.contains('\n') {
        if let Some(first_line) = command.split('\n').find(|l| !l.trim().is_empty()) {
            let trimmed = first_line.trim();
            if !trimmed.is_empty() {
                return suggestion_for_prefix(trimmed);
            }
        }
    }
    if let Some(prefix) = get_simple_command_prefix(command) {
        return suggestion_for_prefix(&prefix);
    }
    vec![allow_local(command.to_string())]
}

/// Generate a prefix rule suggestion (`<prefix>:*`).
///
/// Port of `shellRuleMatching.ts::suggestionForPrefix` (L211-L232) +
/// the bash-side BARE_SHELL_PREFIXES filter (L188-L226).
///
/// **SECURITY**: if `prefix` starts with a bare shell / wrapper /
/// privilege-escalation token, returns an **empty Vec** — never emits
/// `Bash(bash:*)` / `Bash(sudo:*)` / `Bash(env:*)` etc.
pub fn suggestion_for_prefix(prefix: &str) -> Vec<BashRuleSuggestion> {
    if is_bare_shell_prefix(prefix) {
        return Vec::new();
    }
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    vec![allow_local(format!("{trimmed}:*"))]
}
