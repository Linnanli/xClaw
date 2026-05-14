//! Bash permission rule engine — port of upstream `bashPermissions.ts`
//! (2621 LOC) and its shared dependency `shellRuleMatching.ts` (~230 LOC).
//!
//! This is Slice 2.2.a of Phase 2.2 (see
//! `docs/plans/bash-parity/phase-2.2-bash-permissions-rule-engine.md`).
//! Only the rule-string parser + wildcard matcher are landed in this
//! slice; the 8-step authorization pipeline (`bashToolHasPermission`)
//! lands in subsequent slices 2.2.b-h.
//!
//! Upstream canonical source:
//!   `claude-code-main/src/utils/permissions/shellRuleMatching.ts`
//!
//! Non-port deviations (deliberate, see plan §3.1):
//! - upstream regex pipeline is reimplemented as char-by-char Rust loop +
//!   single regex assembly to avoid double escaping; semantics asserted
//!   equivalent by the test matrix
//! - upstream's `caseInsensitive` parameter is preserved as
//!   `MatchOptions::case_insensitive` for forward-compat (Bash itself
//!   is case-sensitive so default is `false`)

pub mod shell_rule_matching;

pub use shell_rule_matching::{
    has_wildcards, match_wildcard_pattern, parse_permission_rule, permission_rule_extract_prefix,
    MatchOptions, ShellPermissionRule,
};
