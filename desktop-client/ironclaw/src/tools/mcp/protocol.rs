//! MCP wire protocol types — **re-export shim**.
//!
//! The canonical implementation lives in the [`dasclaw_mcp::protocol`] crate
//! (F3.2 phase 1, ADR-152 §3). This module is kept so that the dozens of
//! existing `crate::tools::mcp::protocol::*` imports across `ironclaw` keep
//! resolving without a workspace-wide rename. New code should import from
//! `dasclaw_mcp::protocol` directly.

pub use dasclaw_mcp::protocol::*;
