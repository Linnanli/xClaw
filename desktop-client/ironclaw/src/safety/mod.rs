//! Safety layer for prompt injection defense.
//!
//! This module re-exports everything from the `ironclaw_safety` crate,
//! keeping `crate::safety::*` imports working throughout the codebase.
//!
//! The [`egress`] sub-module provides the ADR-148 R2 helper
//! [`egress::sanitize_tool_output_via_egress`] that replaces nine legacy
//! `SafetyLayer::sanitize_tool_output(...).content` call sites with a
//! single Layer-B gate invocation.

pub mod egress;

pub use ironclaw_safety::*;
