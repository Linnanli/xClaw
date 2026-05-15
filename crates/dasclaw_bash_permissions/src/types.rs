//! Shared permission types — semantic port of upstream
//! `claude-code-main/src/types/permissions.ts` (L50-L450 region).
//!
//! Scope of this Slice 2.2.b port:
//! - [`PermissionBehavior`] — 3-variant `Allow|Ask|Deny`
//! - [`PermissionRuleSource`] — 8 origin tags (where a rule was loaded from)
//! - [`PermissionRuleValue`] / [`PermissionRule`] — rule shape
//! - [`ToolPermissionContext`] — bash-relevant subset of upstream context
//! - [`PermissionResult`] — `Deny | Ask | Allow | Passthrough` with rule
//!   attribution (the `decisionReason: { type: 'rule', rule }` variant of
//!   upstream `PermissionDecisionReason`)
//!
//! Deliberate non-port (matches plan §5):
//! - Bash-classifier / pendingClassifierCheck / hooks / sandboxOverride
//!   reasons are NOT ported here (forbidden by Issue #490 epic and/or
//!   deferred to later slices).
//! - `additionalWorkingDirectories` / `isBypassPermissionsModeAvailable` /
//!   `strippedDangerousRules` etc. are deferred to slice 2.2.e (compound
//!   command + env stripping) where they actually become load-bearing.
//! - `metadata` / `contentBlocks` / `blockedPath` / `toolUseID` etc. are
//!   message-level concerns that belong to the eventual `BashPermissionHook`
//!   adapter (slice 2.2.f), not the rule-engine core.
//!
//! All maps use `BTreeMap` for deterministic iteration order — critical
//! because the FIRST matching rule per behavior wins (upstream picks
//! `matchingDenyRules[0]`), and undefined ordering would break
//! reproducibility of the audit-log `rule_id`.

use std::collections::BTreeMap;

/// Permission behavior verb. Maps to upstream
/// `PermissionBehavior = 'allow' | 'ask' | 'deny'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionBehavior {
    Allow,
    Ask,
    Deny,
}

/// Where a permission rule originated from. Semantic port of upstream
/// `PermissionRuleSource` (L54-L65 of `types/permissions.ts`). The 8
/// variants are preserved verbatim — downstream slices (2.2.f, 2.2.h)
/// will surface this in audit logs + UI suggestions, and we don't want
/// to collapse `localSettings` into `projectSettings` (different trust
/// posture) or `flagSettings` into `cliArg` (different lifetime).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

/// What a permission rule targets. Upstream `PermissionRuleValue`.
///
/// For Bash, `tool_name = "Bash"` and `rule_content` carries the
/// raw rule string (e.g. `"npm:*"`, `"git status"`, `"docker *"`) —
/// the rule-string format is parsed by
/// [`crate::shell_rule_matching::parse_permission_rule`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionRuleValue {
    pub tool_name: String,
    pub rule_content: Option<String>,
}

/// A permission rule with its source + behavior. Upstream `PermissionRule`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub rule_behavior: PermissionBehavior,
    pub rule_value: PermissionRuleValue,
}

/// Permission decision reason — bash-engine-relevant subset of upstream
/// `PermissionDecisionReason`. Slice 2.2.b only emits the `Rule` variant
/// (rule-driven matches). Other variants (`Mode`, `Hook`, `Classifier`,
/// `Other`, …) will be added by their respective owning slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecisionReason {
    /// A specific permission rule matched the input.
    Rule { rule: PermissionRule },
    /// Catch-all — no rule matched and no other mechanism opined.
    /// Used by the `Passthrough` result to carry a free-form note.
    Other { reason: String },
}

/// Outcome of running the permission pipeline against a single command.
///
/// Mirrors upstream `PermissionResult`'s discriminated union over
/// `behavior: 'allow' | 'ask' | 'deny' | 'passthrough'`.
///
/// **Precedence** (asserted by [`crate::exact_match::check_exact_match`]):
///
///   `Deny` > `Ask` > `Allow` > `Passthrough`
///
/// This is the same order upstream applies at L996-L1042 of `bashPermissions.ts`:
/// deny rules are checked first (defense-in-depth, refuse before asking
/// or allowing), then ask, then allow. Passthrough means *no engine-level
/// rule fired* — the caller (hook adapter in slice 2.2.f) decides what
/// to do next (typically: ask the user, run classifier, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionResult {
    /// Command refused by a deny rule. The `rule` field tells the audit log
    /// which rule was the match (critical for `rule_id` surface — see
    /// existing `bash_security::block` pinning tests).
    Deny {
        message: String,
        reason: PermissionDecisionReason,
    },
    /// Command requires user approval (matched an `ask` rule).
    Ask {
        message: String,
        reason: PermissionDecisionReason,
    },
    /// Command allowed by an explicit allow rule.
    Allow { reason: PermissionDecisionReason },
    /// No rule matched — caller decides next step. Upstream populates
    /// `suggestions` here to recommend an exact-match rule to the user;
    /// that suggestion logic lives in slice 2.2.g.
    Passthrough {
        message: String,
        reason: PermissionDecisionReason,
    },
}

/// Per-behavior rule storage, grouped by [`PermissionRuleSource`]. Upstream
/// `ToolPermissionRulesBySource`. Stores raw rule-content strings only;
/// they are parsed lazily by the engine via
/// [`crate::shell_rule_matching::parse_permission_rule`].
///
/// `BTreeMap` (not `HashMap`) so iteration is **deterministic across runs**
/// for any given source set — critical for reproducible audit-log
/// `rule_id` ordering.
pub type ToolPermissionRulesBySource = BTreeMap<PermissionRuleSource, Vec<String>>;

/// Context the bash permission pipeline reads from. Bash-relevant subset
/// of upstream `ToolPermissionContext`.
///
/// Per-slice fields actually used by `check_exact_match` (this slice):
/// - [`Self::always_deny_rules`] — checked first
/// - [`Self::always_ask_rules`] — checked second
/// - [`Self::always_allow_rules`] — checked third
///
/// Fields deferred to later slices (kept absent here to avoid premature
/// API commitment): `mode`, `additionalWorkingDirectories`,
/// `isBypassPermissionsModeAvailable`, `strippedDangerousRules`, etc.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPermissionContext {
    pub always_allow_rules: ToolPermissionRulesBySource,
    pub always_deny_rules: ToolPermissionRulesBySource,
    pub always_ask_rules: ToolPermissionRulesBySource,
}

impl ToolPermissionContext {
    /// Convenience builder used by tests + slice 2.2.f wire-up. Adds a
    /// single rule content string under a given source + behavior.
    pub fn add_rule(
        &mut self,
        behavior: PermissionBehavior,
        source: PermissionRuleSource,
        rule_content: impl Into<String>,
    ) {
        let bucket = match behavior {
            PermissionBehavior::Allow => &mut self.always_allow_rules,
            PermissionBehavior::Ask => &mut self.always_ask_rules,
            PermissionBehavior::Deny => &mut self.always_deny_rules,
        };
        bucket.entry(source).or_default().push(rule_content.into());
    }
}
