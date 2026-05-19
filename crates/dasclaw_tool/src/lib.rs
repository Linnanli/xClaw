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

pub mod error;

pub use error::ToolError;
