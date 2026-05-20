//! Shared MCP transport trait and JSON-RPC framing helpers — ironclaw host shim.
//!
//! The host-agnostic implementation (the `McpTransport` trait,
//! `write_jsonrpc_line`, `spawn_jsonrpc_reader`, and the
//! `stream_transport_send` helper shared by stream-based transports) lives
//! in [`dasclaw_mcp::transport`]. This module is kept so that the existing
//! `crate::tools::mcp::transport::*` imports — particularly from the
//! still-local `stdio_transport` and `unix_transport` modules — keep
//! resolving without a workspace-wide rename. New code should import from
//! `dasclaw_mcp::transport` directly.
//!
//! Migrated in F3.2 phase 3 PR 6-α (#661).

pub use dasclaw_mcp::transport::{McpTransport, spawn_jsonrpc_reader, stream_transport_send};
