//! Tool-level feature flags.
//!
//! Provides a lightweight mechanism to enable/disable individual tools at
//! runtime without unregistering them from the registry. Disabled tools are
//! rejected at the `execute_tool_with_safety` gate and excluded from LLM tool
//! definitions so the model never attempts to call them.

use std::collections::HashSet;
use std::sync::Arc;

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
