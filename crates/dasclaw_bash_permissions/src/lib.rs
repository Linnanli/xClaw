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

pub mod compound_match;
pub mod dangerous_patterns;
pub mod exact_match;
pub(crate) mod pipeline;
pub mod prefix_match;
pub mod rule_parser;
pub mod shadowed_rule_detection;
pub mod shell_rule_matching;
pub mod strip_env;
pub mod types;

pub use compound_match::{check_compound_match, MAX_SUBCOMMANDS_FOR_SECURITY_CHECK};
pub use dangerous_patterns::{
    dangerous_bash_patterns, is_dangerous_bash_allow_rule, is_dangerous_bash_allow_rule_value,
    CROSS_PLATFORM_CODE_EXEC,
};
pub use exact_match::{check_exact_match, BASH_TOOL_NAME};
pub use prefix_match::check_prefix_match;
pub use rule_parser::{
    escape_rule_content, get_legacy_tool_names, normalize_legacy_tool_name,
    permission_rule_value_from_string, permission_rule_value_to_string, unescape_rule_content,
};
pub use shadowed_rule_detection::{
    detect_unreachable_rules, is_shared_setting_source, permission_rule_source_display_string,
    DetectUnreachableRulesOptions, ShadowType, UnreachableRule,
};
pub use shell_rule_matching::{
    has_wildcards, match_wildcard_pattern, parse_permission_rule, permission_rule_extract_prefix,
    MatchOptions, ShellPermissionRule,
};
pub use strip_env::{
    is_binary_hijack_var, strip_all_leading_env_vars, strip_safe_wrappers, SAFE_ENV_VARS,
};
pub use types::{
    PermissionBehavior, PermissionDecisionReason, PermissionResult, PermissionRule,
    PermissionRuleSource, PermissionRuleValue, ToolPermissionContext, ToolPermissionRulesBySource,
};
