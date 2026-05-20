//! MCP stdio process manager — ironclaw host shim.
//!
//! The host-agnostic implementation lives in [`dasclaw_mcp::process`].
//! `McpProcessManager` is re-exported because external ironclaw modules
//! (`extensions::manager`, `channels::web::server`,
//! `tools::builtin::extension_tools`) still resolve it through this path.
//! `StdioSpawnConfig` is re-exported under `allow(unused_imports)` for
//! parity even if no external caller currently uses it.
//!
//! Migrated in F3.2 phase 3 PR 6-β (#661, #664).

pub use dasclaw_mcp::process::McpProcessManager;
#[allow(unused_imports)]
pub use dasclaw_mcp::process::StdioSpawnConfig;
