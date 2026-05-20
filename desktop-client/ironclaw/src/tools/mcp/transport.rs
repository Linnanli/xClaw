//! Shared MCP transport trait — ironclaw host shim.
//!
//! The host-agnostic implementation lives in [`dasclaw_mcp::transport`].
//! `McpTransport` is re-exported because `mcp::client` still resolves it
//! through this path. `spawn_jsonrpc_reader` was needed by the stdio /
//! unix transports while they lived in ironclaw; after PR 6-β they live in
//! `dasclaw_mcp`, so it is re-exported under `allow(unused_imports)` for
//! parity. `stream_transport_send` was tightened back to `pub(crate)` in
//! `dasclaw_mcp::transport` and is no longer re-exported here.
//!
//! Migrated in F3.2 phase 3 PR 6-α (#661, #663); shim trimmed in PR 6-β (#664).

pub use dasclaw_mcp::transport::McpTransport;
#[allow(unused_imports)]
pub use dasclaw_mcp::transport::spawn_jsonrpc_reader;
