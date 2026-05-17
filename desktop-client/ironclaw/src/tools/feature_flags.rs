//! Tool-level feature flags.
//!
//! Provides a lightweight mechanism to enable/disable individual tools at
//! runtime without unregistering them from the registry. Disabled tools are
//! rejected at the `execute_tool_with_safety` gate and excluded from LLM tool
//! definitions so the model never attempts to call them.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_governance::tool_visibility::{
    ToolGateContext, ToolGateDecision, ToolSource, ToolVisibilityPolicy,
};

/// Tool-level feature flag configuration.
///
/// Loaded from `Config.feature_flags` and optionally updated via Admin Backend
/// `system_settings` KV pushes. The default state is **all tools enabled** —
/// only tools explicitly listed in `disabled_tools` are rejected.
#[derive(Debug, Clone)]
pub struct ToolFeatureFlags {
    disabled_tools: HashSet<String>,
}

impl ToolFeatureFlags {
    /// Create a new instance with all tools enabled.
    pub fn all_enabled() -> Self {
        Self {
            disabled_tools: HashSet::new(),
        }
    }

    /// Create from a set of disabled tool names.
    pub fn with_disabled(disabled: impl IntoIterator<Item = String>) -> Self {
        Self {
            disabled_tools: disabled.into_iter().collect(),
        }
    }

    /// Check whether a tool is enabled.
    ///
    /// Returns `true` unless the tool name is in the disabled set.
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        !self.disabled_tools.contains(tool_name)
    }

    /// Return the set of disabled tool names (for diagnostics / filtering).
    pub fn disabled_tools(&self) -> &HashSet<String> {
        &self.disabled_tools
    }
}

impl Default for ToolFeatureFlags {
    fn default() -> Self {
        Self::all_enabled()
    }
}

/// Convenience alias used by `JobContext` and callers that share the flags
/// across cloned contexts cheaply.
pub type SharedFeatureFlags = Arc<ToolFeatureFlags>;

/// ADR-149 / issue #485 — built-in tool blocklist as a
/// [`ToolVisibilityPolicy`].
///
/// Adapts the existing [`ToolFeatureFlags`] HashSet into the triple-gate
/// policy surface so the L2 (LLM definitions) and L3 (executor preflight)
/// gates share a single source of truth. The contract is intentionally
/// narrow:
///
/// * Only [`ToolSource::BuiltIn`] tools are evaluated against the
///   blocklist. MCP / Skill / Extension / Wasm tools always pass — they
///   have separate gating mechanisms (extension manifest signing, MCP
///   server allowlist, etc.) that will land in dedicated policies.
/// * A disabled BuiltIn produces [`ToolGateDecision::Hide`] (cache-busting
///   at L2; surfaced as `ToolError::Disabled` at L3). The `reason` field
///   names the tool so audit logs and error messages can identify it.
/// * An enabled BuiltIn (or any non-BuiltIn) produces
///   [`ToolGateDecision::Allow`].
///
/// **Migration parity**: `BlocklistPolicy::new(flags).check(name@BuiltIn)`
/// is observationally equivalent to `flags.is_tool_enabled(name)` for
/// every name × disabled-set combination — see
/// `req_tool_visibility_84_020_blocklist_migration_parity`.
#[derive(Debug, Clone)]
pub struct BlocklistPolicy {
    flags: SharedFeatureFlags,
}

impl BlocklistPolicy {
    /// Wrap an existing [`SharedFeatureFlags`] reference. The policy holds
    /// a clone of the [`Arc`], so updates to the underlying flags require
    /// either swapping the policy on the registry or making
    /// [`ToolFeatureFlags`] interior-mutable in a future revision.
    pub fn new(flags: SharedFeatureFlags) -> Self {
        Self { flags }
    }
}

#[async_trait]
impl ToolVisibilityPolicy for BlocklistPolicy {
    async fn check(&self, ctx: &ToolGateContext<'_>) -> ToolGateDecision {
        // Only BuiltIn tools are governed by the legacy blocklist; every
        // other source has its own gate (extension signing, MCP allowlist,
        // wasm sealed-tool verifier — issue #484+).
        if !matches!(ctx.source, ToolSource::BuiltIn) {
            return ToolGateDecision::Allow;
        }
        if self.flags.is_tool_enabled(ctx.tool_name) {
            ToolGateDecision::Allow
        } else {
            ToolGateDecision::Hide {
                reason: format!(
                    "tool '{}' is disabled by feature flag (BlocklistPolicy)",
                    ctx.tool_name
                ),
            }
        }
    }

    fn name(&self) -> &str {
        "ironclaw::BlocklistPolicy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_enabled_allows_any_tool() {
        let flags = ToolFeatureFlags::all_enabled();
        assert!(flags.is_tool_enabled("echo"));
        assert!(flags.is_tool_enabled("shell"));
        assert!(flags.is_tool_enabled("nonexistent_tool"));
    }

    #[test]
    fn disabled_tool_is_rejected() {
        let flags = ToolFeatureFlags::with_disabled(vec!["shell".into(), "http".into()]);
        assert!(!flags.is_tool_enabled("shell"));
        assert!(!flags.is_tool_enabled("http"));
        assert!(flags.is_tool_enabled("echo"));
    }

    #[test]
    fn default_is_all_enabled() {
        let flags = ToolFeatureFlags::default();
        assert!(flags.is_tool_enabled("anything"));
    }

    #[test]
    fn disabled_tools_accessor() {
        let flags = ToolFeatureFlags::with_disabled(vec!["a".into(), "b".into()]);
        let set = flags.disabled_tools();
        assert_eq!(set.len(), 2);
        assert!(set.contains("a"));
        assert!(set.contains("b"));
    }
}
