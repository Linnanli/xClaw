//! LSP integration host wiring.
//!
//! Both the core LSP types (registry, JSON-RPC client, protocol, server
//! config) and the host-facing [`LspQueryTool`] now live in [`dasclaw_lsp`]
//! (ADR-152 §3 F3.3, issue #688). This module is a thin re-export so every
//! existing `crate::tools::builtin::lsp::*` path keeps resolving.

pub use dasclaw_lsp::{
    LspAction, LspClient, LspQueryTool, LspRegistry, LspServerConfig, LspServerMapping, path_to_uri,
};
