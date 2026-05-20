//! LSP integration: registry, JSON-RPC client, protocol & server mapping.
//!
//! Verbatim port from `desktop-client/ironclaw/src/tools/builtin/lsp/` per
//! F3.3 (#629). The host-side `LspQueryTool` (the [`Tool`]-trait wiring)
//! remains in `ironclaw::tools::builtin::lsp` until the [`Tool`] trait moves
//! into `dasclaw_tool` in a follow-up issue.
//!
//! ```text
//! LspRegistry (host-owned)
//!     ├─ language → LspClient (lazy start)
//!     ├─ idle timeout → auto shutdown
//!     └─ Admin whitelist filtering
//! ```
//!
//! [`Tool`]: https://github.com/Linnanli/x-claw — see `crate::tools::tool::Tool` in the ironclaw crate

mod client;
pub(crate) mod protocol;
mod registry;
mod server_config;

pub use client::{LspClient, path_to_uri};
pub use protocol::LspAction;
pub use registry::LspRegistry;
pub use server_config::{LspServerConfig, LspServerMapping};
