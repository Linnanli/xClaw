//! MCP session management — ironclaw host shim.
//!
//! The host-agnostic `McpSession` / `McpSessionManager` implementation
//! lives in [`dasclaw_mcp::session`]. This module is kept so that the
//! existing `crate::tools::mcp::session::*` imports keep resolving
//! without a workspace-wide rename. New code should import from
//! `dasclaw_mcp::session` directly.
//!
//! Migrated in F3.2 phase 3 PR 6-α (#661).

pub use dasclaw_mcp::session::{McpSession, McpSessionManager};
