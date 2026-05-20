//! Stdio MCP transport — ironclaw host shim.
//!
//! The host-agnostic implementation lives in
//! [`dasclaw_mcp::stdio_transport`]. No code outside the `mcp` module
//! currently imports from this path; the file is kept as a placeholder so
//! that future ironclaw code can still write
//! `crate::tools::mcp::stdio_transport::StdioMcpTransport` without
//! triggering an unused-import warning if no caller actually exists.
//!
//! Migrated in F3.2 phase 3 PR 6-β (#661, #664).

#[allow(unused_imports)]
pub use dasclaw_mcp::stdio_transport::StdioMcpTransport;
