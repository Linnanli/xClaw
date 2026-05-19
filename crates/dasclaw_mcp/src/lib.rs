//! Model Context Protocol (MCP) building blocks shared across dasclaw hosts.
//!
//! ## Status (F3.2 phase 2)
//!
//! Phase 1 ported the wire-protocol surface (`protocol`, `server_name`).
//! Phase 2 sub-PR 5a (#641) adds **`config`** — the host-agnostic
//! `McpServerConfig` / `OAuthConfig` / `McpServersFile` data types,
//! validation, and explicit-path JSON load/save.
//!
//! Still in the ironclaw host crate (planned for the remaining sub-PRs):
//! `auth`, `client`, `*_transport`, `session`, `process`, `factory`, plus
//! the ironclaw-specific default-path / database-backed config wrappers.
//!
//! See `gh issue view 628`, ADR-152 §3 phase F3.2, and PR #641 for context.

pub mod config;
pub mod protocol;
pub mod server_name;

pub use config::{
    is_localhost_url, load_mcp_servers_from, save_mcp_servers_to, ConfigError, EffectiveTransport,
    McpServerConfig, McpServersFile, McpTransportConfig, OAuthConfig,
};
pub use protocol::{
    CallToolResult, ContentBlock, ExecutionTimeHint, InitializeResult, ListToolsResult, McpError,
    McpRequest, McpResponse, McpTool, McpToolAnnotations, PromptsCapability, ResourcesCapability,
    ServerCapabilities, ServerInfo, ToolsCapability, PROTOCOL_VERSION,
};
pub use server_name::{McpServerName, McpServerNameError};
