//! HTTP (Streamable HTTP / SSE) MCP transport — ironclaw host shim.
//!
//! The host-agnostic implementation lives in
//! [`dasclaw_mcp::http_transport`]. This module re-exports the public
//! surface so existing `crate::tools::mcp::http_transport::*` imports keep
//! resolving without a workspace-wide rename. New code should import from
//! `dasclaw_mcp::http_transport` directly.
//!
//! Migrated in F3.2 phase 3 PR 6-β (#661, #664).

pub use dasclaw_mcp::http_transport::HttpMcpTransport;
