//! Pure-data types shared across dasclaw hosts.
//!
//! This module hosts the data types that previously lived in
//! `desktop-client/ironclaw/src/tools/tool.rs` and that carry **no** dependency
//! on ironclaw internals (no `JobContext`, no `WebhookCapability`). The `Tool`
//! trait itself stays in ironclaw until `JobContext` is decoupled in a
//! follow-up issue.
//!
//! Search history: F3.3 phase 2 PR (#670) verbatim port per ADR-129 §1.3.

use std::fmt;
use std::time::Duration;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// How much approval a specific tool invocation requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequirement {
    /// No approval needed.
    Never,
    /// Needs approval, but session auto-approve can bypass.
    UnlessAutoApproved,
    /// Always needs explicit approval (even if auto-approved).
    Always,
}

impl ApprovalRequirement {
    /// Whether this invocation requires approval in contexts where
    /// auto-approve is irrelevant (e.g. autonomous worker/scheduler).
    pub fn is_required(&self) -> bool {
        !matches!(self, Self::Never)
    }
}

/// Precomputed autonomous tool scope for background jobs and routines.
///
/// Interactive sessions don't use this type — they still rely on
/// `requires_approval()` and session-level approval state.
#[derive(Debug, Clone)]
pub enum ApprovalContext {
    /// Autonomous job with no interactive user. Only tools in `allowed_tools`
    /// may run; interactive approval requirements are ignored.
    Autonomous {
        /// Tool names that may run autonomously for this job/run.
        allowed_tools: std::collections::HashSet<String>,
    },
}

impl ApprovalContext {
    /// Create an autonomous context with no allowed tools.
    pub fn autonomous() -> Self {
        Self::Autonomous {
            allowed_tools: std::collections::HashSet::new(),
        }
    }

    /// Create an autonomous context with specific allowed tools.
    pub fn autonomous_with_tools(tools: impl IntoIterator<Item = String>) -> Self {
        Self::Autonomous {
            allowed_tools: tools.into_iter().collect(),
        }
    }

    /// Check whether a tool invocation is blocked in this context.
    pub fn is_blocked(&self, tool_name: &str, _requirement: ApprovalRequirement) -> bool {
        match self {
            Self::Autonomous { allowed_tools } => !allowed_tools.contains(tool_name),
        }
    }

    /// Check whether a tool is blocked given an optional context.
    ///
    /// When `None`, falls back to legacy behavior: all non-`Never` tools are blocked.
    pub fn is_blocked_or_default(
        context: &Option<Self>,
        tool_name: &str,
        requirement: ApprovalRequirement,
    ) -> bool {
        match context {
            Some(ctx) => ctx.is_blocked(tool_name, requirement),
            None => requirement.is_required(),
        }
    }
}

/// Per-tool rate limit configuration for built-in tool invocations.
///
/// Controls how many times a tool can be invoked per user, per time window.
/// Read-only tools (echo, time, json, file_read, etc.) should NOT be rate limited.
/// Write/external tools (shell, http, file_write, memory_write, create_job) should be.
#[derive(Debug, Clone)]
pub struct ToolRateLimitConfig {
    /// Maximum invocations per minute.
    pub requests_per_minute: u32,
    /// Maximum invocations per hour.
    pub requests_per_hour: u32,
}

impl ToolRateLimitConfig {
    /// Create a config with explicit limits.
    pub fn new(requests_per_minute: u32, requests_per_hour: u32) -> Self {
        Self {
            requests_per_minute,
            requests_per_hour,
        }
    }
}

impl Default for ToolRateLimitConfig {
    /// Default: 60 requests/minute, 1000 requests/hour (generous for WASM HTTP).
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            requests_per_hour: 1000,
        }
    }
}

/// Risk level of a tool invocation.
///
/// Used by the shell tool to classify commands and by the worker to drive
/// approval decisions and observability logging. Implements `Ord` so callers
/// can compare levels (e.g. `risk >= RiskLevel::High`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Read-only, safe, reversible (e.g. `ls`, `cat`, `grep`).
    Low,
    /// Creates or modifies state, but generally reversible
    /// (e.g. `mkdir`, `git commit`, `cargo build`).
    Medium,
    /// Destructive, irreversible, or security-sensitive
    /// (e.g. `rm -rf`, `git push --force`, `kill -9`).
    High,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => f.write_str("low"),
            Self::Medium => f.write_str("medium"),
            Self::High => f.write_str("high"),
        }
    }
}

/// Where a tool should execute: orchestrator process or inside a container.
///
/// Orchestrator tools run in the main agent process (memory access, job mgmt, etc).
/// Container tools run inside Docker containers (shell, file ops, code mods).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolDomain {
    /// Safe to run in the orchestrator (pure functions, memory, job management).
    Orchestrator,
    /// Must run inside a sandboxed container (filesystem, shell, code).
    Container,
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The result data.
    pub result: serde_json::Value,
    /// Cost incurred (if any).
    pub cost: Option<Decimal>,
    /// Time taken.
    pub duration: Duration,
    /// Raw output before sanitization (for debugging).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

impl ToolOutput {
    /// Create a successful output with a JSON result.
    pub fn success(result: serde_json::Value, duration: Duration) -> Self {
        Self {
            result,
            cost: None,
            duration,
            raw: None,
        }
    }

    /// Create a text output.
    pub fn text(text: impl Into<String>, duration: Duration) -> Self {
        Self {
            result: serde_json::Value::String(text.into()),
            cost: None,
            duration,
            raw: None,
        }
    }

    /// Set the cost.
    pub fn with_cost(mut self, cost: Decimal) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Set the raw output.
    pub fn with_raw(mut self, raw: impl Into<String>) -> Self {
        self.raw = Some(raw.into());
        self
    }
}

/// Definition of a tool's parameters using JSON Schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolSchema {
    /// Create a new tool schema.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    /// Set the parameters schema.
    pub fn with_parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = parameters;
        self
    }
}

/// Curated discovery guidance surfaced by `tool_info(detail: "summary")`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolDiscoverySummary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditional_requirements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_context_autonomous_blocks_tools_not_in_scope() {
        let ctx = ApprovalContext::autonomous();
        assert!(ctx.is_blocked("shell", ApprovalRequirement::Never));
        assert!(ctx.is_blocked("shell", ApprovalRequirement::UnlessAutoApproved));
        assert!(ctx.is_blocked("shell", ApprovalRequirement::Always));
    }

    #[test]
    fn test_approval_context_autonomous_with_tools_allows_registered_name() {
        let ctx =
            ApprovalContext::autonomous_with_tools(["shell".to_string(), "message".to_string()]);
        assert!(!ctx.is_blocked("shell", ApprovalRequirement::Never));
        assert!(!ctx.is_blocked("shell", ApprovalRequirement::Always));
        assert!(!ctx.is_blocked("message", ApprovalRequirement::Always));
        assert!(ctx.is_blocked("http", ApprovalRequirement::Always));
    }

    #[test]
    fn test_approval_context_blocks_never_when_not_in_scope() {
        let ctx = ApprovalContext::autonomous();
        assert!(ctx.is_blocked("any_tool", ApprovalRequirement::Never));
    }

    #[test]
    fn test_is_blocked_or_default_with_none_uses_legacy() {
        // None context: all non-Never tools are blocked
        assert!(!ApprovalContext::is_blocked_or_default(
            &None,
            "any",
            ApprovalRequirement::Never
        ));
        assert!(ApprovalContext::is_blocked_or_default(
            &None,
            "any",
            ApprovalRequirement::UnlessAutoApproved
        ));
        assert!(ApprovalContext::is_blocked_or_default(
            &None,
            "any",
            ApprovalRequirement::Always
        ));
    }

    #[test]
    fn test_is_blocked_or_default_with_some_delegates() {
        let ctx = Some(ApprovalContext::autonomous_with_tools(
            ["shell".to_string()],
        ));
        assert!(!ApprovalContext::is_blocked_or_default(
            &ctx,
            "shell",
            ApprovalRequirement::Always
        ));
        assert!(ApprovalContext::is_blocked_or_default(
            &ctx,
            "other",
            ApprovalRequirement::Always
        ));
        assert!(ApprovalContext::is_blocked_or_default(
            &ctx,
            "any",
            ApprovalRequirement::UnlessAutoApproved
        ));
    }
}
