//! Tool-level feature-flag vocabulary shared across dasclaw hosts.
//!
//! The bare data type [`ToolFeatureFlags`] and its convenience alias
//! [`SharedFeatureFlags`] were verbatim-ported from
//! `desktop-client/ironclaw/src/tools/feature_flags.rs` (ADR-129 §1.3,
//! ADR-154 step 1/2). The `BuiltinBlocklistPolicy` adapter (which depends
//! on `dasclaw_governance::tool_visibility`) stays in ironclaw — only the
//! HashSet-backed vocabulary moves down so that the future
//! [`crate::job_context::JobContextCore`] trait can expose
//! `SharedFeatureFlags` without dragging governance into this crate.

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
