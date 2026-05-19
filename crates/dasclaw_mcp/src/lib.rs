//! Model Context Protocol (MCP) building blocks shared across dasclaw hosts.
//!
//! ## Status (F3.2 phase 1)
//!
//! Phase 1 ports the **pure** parts of the MCP stack out of
//! `desktop-client/ironclaw/src/tools/mcp/`:
//!
//! - [`protocol`] — wire types (JSON-RPC framing, `McpTool`, `InitializeResult`, …)
//! - [`server_name`] — typed [`server_name::McpServerName`] with the alnum allowlist
//!   that the x-claw fork dropped during the rename pass (security delta)
//!
//! `auth`, `client`, `config`, `*_transport`, `session`, `process`, `factory`
//! still live in the ironclaw host crate; they will move in follow-up phases.
//!
//! See `gh issue view 628` and ADR-152 §3 phase F3.2 for the rationale.

pub mod protocol;
pub mod server_name;

pub use protocol::{
    CallToolResult, ContentBlock, ExecutionTimeHint, InitializeResult, ListToolsResult, McpError,
    McpRequest, McpResponse, McpTool, McpToolAnnotations, PromptsCapability, ResourcesCapability,
    ServerCapabilities, ServerInfo, ToolsCapability, PROTOCOL_VERSION,
};
pub use server_name::{McpServerName, McpServerNameError};
