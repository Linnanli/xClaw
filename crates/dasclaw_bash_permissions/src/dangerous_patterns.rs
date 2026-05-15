//! Dangerous bash allow-rule predicate — port of upstream
//! `claude-code-main/src/utils/permissions/dangerousPatterns.ts`
//! (L17-L42 `CROSS_PLATFORM_CODE_EXEC` + L45-L80 `DANGEROUS_BASH_PATTERNS`)
//! and `permissionSetup.ts` (L94-L147 `isDangerousBashPermission`).
//!
//! ## Purpose
//!
//! Allow rules like `Bash(python:*)` or `Bash(node*)` give the model a free
//! pass to execute arbitrary code through that interpreter, bypassing every
//! downstream safety check (compound split, env-var stripping, deny rules,
//! sandbox). This module is the **rule-loading safety filter**: hosts call
//! [`is_dangerous_bash_allow_rule`] before promoting a rule to active state
//! and **must refuse** rules that match. (Slice 2.2.h will wire it into
//! `permissions_loader`; this slice ships the predicate + tests only.)
//!
//! ## Scope (Slice 2.2.g)
//!
//! - `DANGEROUS_BASH_PATTERNS` external-user subset only
//! - `is_dangerous_bash_allow_rule(tool, content)` predicate
//!
//! ## What is NOT in this slice
//!
//! - **`ANT_ONLY` patterns** (upstream L57-L78: `fa run`, `coo`, `gh`,
//!   `gh api`, `curl`, `wget`, `git`, `kubectl`, `aws`, `gcloud`, `gsutil`):
//!   Issue #490 epic explicitly forbids ANT-only porting. The set is empty
//!   for x-claw (`USER_TYPE === 'ant'` branch hard-coded false). Test 11
//!   pins this regression-defense.
//! - **PowerShell**: separate `isDangerousPowerShellPermission` upstream
//!   (L161+); out of scope for the bash-parity epic. Track separately.
//! - **Auto-mode entry stripping**: upstream `permissionSetup.ts` L281+
//!   uses the predicate to filter rules at auto-mode entry. This slice
//!   ships the predicate; wiring is Slice 2.2.h.

use crate::exact_match::BASH_TOOL_NAME;
use crate::types::PermissionRuleValue;

/// Cross-platform code-exec interpreters + package runners + shells +
/// ssh — upstream `CROSS_PLATFORM_CODE_EXEC` (L17-L42).
///
/// Allowing any of these as a broad rule (`python:*`, `npm run*`,
/// `node *`) lets the model execute arbitrary code through them,
/// bypassing every downstream check.
pub const CROSS_PLATFORM_CODE_EXEC: &[&str] = &[
    // Interpreters (upstream L19-L29)
    "python", "python3", "python2", "node", "deno", "tsx", "ruby", "perl", "php", "lua",
    // Package runners (upstream L30-L36)
    "npx", "bunx", "npm run", "yarn run", "pnpm run", "bun run",
    // Shells reachable from both (upstream L37-L40)
    "bash", "sh", // Remote arbitrary-command wrapper (upstream L41-L42)
    "ssh",
];

/// Non-cross-platform but still cross-environment dangerous patterns —
/// upstream `DANGEROUS_BASH_PATTERNS` L45-L56 (excluding ANT-only L57-L78).
///
/// **SECURITY PIN**: ANT-only patterns are INTENTIONALLY EXCLUDED. Issue
/// #490 explicitly forbids porting ANT-only constants. Adding any of
/// `fa run` / `coo` / `gh` / `gh api` / `curl` / `wget` / `git` /
/// `kubectl` / `aws` / `gcloud` / `gsutil` here violates the epic's red
/// line. Test 11 pins this guard.
const EXTRA_BASH_PATTERNS: &[&str] = &[
    // Shells (upstream L46-L47)
    "zsh", "fish", // Direct shell exec primitives (upstream L48-L50)
    "eval", "exec", "env", // Argv-feeders (upstream L51)
    "xargs", // Privilege escalation (upstream L52)
    "sudo",
];

/// Iterator over **all** dangerous bash patterns for external (non-ANT)
/// users. Order is preserved per upstream for log/diff stability.
pub fn dangerous_bash_patterns() -> impl Iterator<Item = &'static &'static str> {
    CROSS_PLATFORM_CODE_EXEC
        .iter()
        .chain(EXTRA_BASH_PATTERNS.iter())
}

/// Predicate counterpart of upstream `isDangerousBashPermission`
/// (`permissionSetup.ts` L94-L147).
///
/// Returns `true` when the (tool, rule-content) pair would grant the
/// model an arbitrary-code-execution shortcut and must be refused at
/// rule-load time.
///
/// Match semantics (upstream L117-L145):
/// - tool name must be the Bash tool
/// - `None` / `""` rule content → tool-level allow → dangerous
/// - rule content `*` → universal wildcard → dangerous
/// - for each dangerous pattern (lowercased):
///   * exact match → dangerous
///   * `pattern:*` → dangerous
///   * `pattern*` → dangerous
///   * `pattern *` → dangerous
///   * starts with `pattern -` AND ends with `*` → dangerous (e.g.
///     `python -c*`, `python -m*`)
///
/// Content comparison is **case-insensitive after trim**, matching
/// upstream L106 `content.trim().toLowerCase()`.
pub fn is_dangerous_bash_allow_rule(tool_name: &str, rule_content: Option<&str>) -> bool {
    // Only Bash rules (upstream L95-L97).
    if tool_name != BASH_TOOL_NAME {
        return false;
    }

    let Some(raw) = rule_content else {
        // Tool-level allow `Bash` with no content (upstream L100-L102).
        return true;
    };
    if raw.is_empty() {
        return true;
    }

    let content = raw.trim().to_lowercase();
    if content.is_empty() {
        // `Bash("   ")` — trim-equivalent to no content (defensive).
        return true;
    }

    // Standalone wildcard (upstream L109-L111).
    if content == "*" {
        return true;
    }

    for pattern in dangerous_bash_patterns() {
        let p = pattern.to_lowercase();
        // Exact (upstream L120-L122)
        if content == p {
            return true;
        }
        // `pattern:*` (upstream L125-L127)
        if content == format!("{}:*", p) {
            return true;
        }
        // `pattern*` (upstream L130-L132)
        if content == format!("{}*", p) {
            return true;
        }
        // `pattern *` (upstream L135-L137)
        if content == format!("{} *", p) {
            return true;
        }
        // `pattern -...*` (upstream L140-L142)
        let dash_prefix = format!("{} -", p);
        if content.starts_with(&dash_prefix) && content.ends_with('*') {
            return true;
        }
    }

    false
}

/// Convenience wrapper for callers holding a [`PermissionRuleValue`].
/// Extracts `tool_name` + `rule_content` and forwards to
/// [`is_dangerous_bash_allow_rule`].
pub fn is_dangerous_bash_allow_rule_value(value: &PermissionRuleValue) -> bool {
    is_dangerous_bash_allow_rule(&value.tool_name, value.rule_content.as_deref())
}
