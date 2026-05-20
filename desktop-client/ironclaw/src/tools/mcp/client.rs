//! Shim for the migrated `McpClient`.
//!
//! `McpClient` and its `RefreshAccessTokenFn` type alias moved to
//! [`dasclaw_mcp::client`] in F3.2 phase 3 PR 6-γ (#661, #666). This shim
//! preserves the `crate::tools::mcp::client::McpClient` path used across
//! ironclaw (factory, channels, app) so the migration is a verbatim port.
//!
//! The host-specific `Tool` adapter (`McpToolWrapper`, `create_tools_for`,
//! `ironclaw_refresh_fn`, `strip_top_level_nulls`) lives in
//! [`crate::tools::mcp::client_tool`].

pub use dasclaw_mcp::client::McpClient;
