//! LSP integration: registry, JSON-RPC client, protocol & server mapping,
//! plus the host-facing `LspQueryTool`.
//!
//! Core types verbatim-ported from `desktop-client/ironclaw/src/tools/builtin/lsp/`
//! per ADR-152 §3 F3.3. The `LspQueryTool` joined them under issue #688 once
//! the `Tool` trait moved to `dasclaw_runtime` (PR #687 / ADR-154 §10).
//!
//! ```text
//! LspRegistry (host-owned)
//!     ├─ language → LspClient (lazy start)
//!     ├─ idle timeout → auto shutdown
//!     └─ Admin whitelist filtering
//! ```

mod client;
pub(crate) mod protocol;
mod registry;
mod server_config;
mod tool;

pub use client::{LspClient, path_to_uri};
pub use protocol::LspAction;
pub use registry::LspRegistry;
pub use server_config::{LspServerConfig, LspServerMapping};
pub use tool::LspQueryTool;
