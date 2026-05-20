//! LSP integration host wiring.
//!
//! Core LSP types (registry, JSON-RPC client, protocol, server config) live
//! in [`dasclaw_lsp`] after F3.3 (#629). The host-side [`LspQueryTool`]
//! (impl [`crate::tools::tool::Tool`]) stays here until the `Tool` trait
//! moves into `dasclaw_tool` in a follow-up issue.

pub use dasclaw_lsp::{
    LspAction, LspClient, LspRegistry, LspServerConfig, LspServerMapping, path_to_uri,
};

mod tool;
pub use tool::LspQueryTool;
