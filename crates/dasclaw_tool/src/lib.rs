//! Tool implementation-layer primitives shared across dasclaw hosts.
//!
//! This crate hosts the **inner** error type that an individual tool's
//! implementation returns. The outer "application layer" error (which adds
//! the failing tool's identity to the wire) lives in a separate crate and
//! is constructed by the dispatcher when a tool error bubbles up.
//!
//! The two-layer split mirrors the design in `ironclaw-main`:
//! `src/tools/tool.rs::ToolError` (this crate) carries no tool name;
//! `src/error.rs::ToolError` (future `dasclaw_runtime`) attaches the name
//! at the call boundary.
//!
//! Beyond `ToolError`, this crate also exposes the **pure-data primitives**
//! (`RiskLevel`, `ToolDomain`, `ApprovalRequirement`, `ApprovalContext`,
//! `ToolOutput`, `ToolSchema`, `ToolDiscoverySummary`, `ToolRateLimitConfig`)
//! and JSON-Schema/parameter helpers (`require_str`, `require_param`,
//! `redact_params`, `validate_tool_schema`) that any dasclaw host needs to
//! describe and dispatch tool invocations. The `Tool` trait itself still
//! lives in ironclaw (it depends on `JobContext` and `WebhookCapability`,
//! which need their own decoupling effort tracked in a follow-up issue).

pub mod error;
pub mod params;
pub mod types;

pub use error::ToolError;
pub use params::{redact_params, require_param, require_str, validate_tool_schema};
pub use types::{
    ApprovalContext, ApprovalRequirement, RiskLevel, ToolDiscoverySummary, ToolDomain, ToolOutput,
    ToolRateLimitConfig, ToolSchema,
};
