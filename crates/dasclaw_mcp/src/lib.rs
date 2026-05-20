//! Model Context Protocol (MCP) building blocks shared across dasclaw hosts.
//!
//! ## Status (F3.2 phase 2 / phase 3)
//!
//! Phase 1 ported the wire-protocol surface (`protocol`, `server_name`).
//! Phase 2 sub-PR 5a (#641) adds **`config`** — the host-agnostic
//! `McpServerConfig` / `OAuthConfig` / `McpServersFile` data types,
//! validation, and explicit-path JSON load/save.
//! Phase 2 sub-PR 5b (#641) adds **`auth`** — the host-agnostic OAuth 2.1
//! protocol layer (PKCE, discovery, DCR, URL building, code exchange,
//! direct refresh, token storage). The ironclaw host keeps the GUI
//! orchestration, localhost callback server, and OAuth-proxy refresh path
//! on top of it.
//! Phase 3 sub-PR 6-α (#661) adds **`session`** (host-agnostic
//! `McpSession` / `McpSessionManager`) and **`transport`** (the
//! `McpTransport` trait plus the JSON-RPC line framing and pending-
//! response dispatch helpers used by stream-based transports).
//!
//! Still in the ironclaw host crate (planned for the remaining sub-PRs):
//! `client`, `stdio_transport`, `unix_transport`, `http_transport`,
//! `process`, `factory`, plus the ironclaw-specific default-path /
//! database-backed config wrappers and the GUI auth orchestration.
//!
//! See `gh issue view 661`, ADR-152 §3 phase F3.2, and PR #641 for context.

pub mod auth;
pub mod config;
pub mod protocol;
pub mod server_name;
pub mod session;
pub mod transport;

pub use auth::{
    AccessToken, AuthError, AuthorizationServerMetadata, ClientCredentials,
    ClientRegistrationRequest, ClientRegistrationResponse, DEFAULT_DCR_CLIENT_NAME, PkceChallenge,
    ProtectedResourceMetadata, build_authorization_url, build_well_known_uri,
    canonical_resource_uri, discover_authorization_server, discover_full_oauth_metadata,
    discover_oauth_endpoints, discover_protected_resource, exchange_code_for_token,
    get_access_token, is_authenticated, refresh_access_token_direct, refresh_lock, register_client,
    store_client_id, store_client_secret, store_tokens, validate_url_safe,
};
pub use config::{
    ConfigError, EffectiveTransport, McpServerConfig, McpServersFile, McpTransportConfig,
    OAuthConfig, is_localhost_url, load_mcp_servers_from, save_mcp_servers_to,
};
pub use protocol::{
    CallToolResult, ContentBlock, ExecutionTimeHint, InitializeResult, ListToolsResult, McpError,
    McpRequest, McpResponse, McpTool, McpToolAnnotations, PROTOCOL_VERSION, PromptsCapability,
    ResourcesCapability, ServerCapabilities, ServerInfo, ToolsCapability,
};
pub use server_name::{McpServerName, McpServerNameError};
pub use session::{McpSession, McpSessionManager};
pub use transport::{
    McpTransport, spawn_jsonrpc_reader, stream_transport_send, write_jsonrpc_line,
};
