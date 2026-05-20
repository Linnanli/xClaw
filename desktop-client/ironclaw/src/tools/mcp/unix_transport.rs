//! Unix domain socket MCP transport — ironclaw host shim.
//!
//! The host-agnostic implementation lives in
//! [`dasclaw_mcp::unix_transport`]. This module re-exports the public
//! surface so existing `crate::tools::mcp::unix_transport::*` imports keep
//! resolving without a workspace-wide rename. New code should import from
//! `dasclaw_mcp::unix_transport` directly.
//!
//! Migrated in F3.2 phase 3 PR 6-β (#661, #664).

#[cfg(unix)]
pub use dasclaw_mcp::unix_transport::UnixMcpTransport;
