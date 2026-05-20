//! Ironclaw factory shim — wraps [`dasclaw_mcp::factory::create_client_from_config`]
//! and installs the ironclaw-specific OAuth-proxy refresh hook.
//!
//! The host-agnostic factory moved to `dasclaw_mcp::factory` in F3.2 phase 3
//! PR 6-γ (#661, #666). All callers keep using `crate::tools::mcp::factory`
//! so they don't need to know about the host hook; the wrapper here calls
//! [`crate::tools::mcp::client_tool::ironclaw_refresh_fn`] to install the
//! orchestration wrapper around [`crate::tools::mcp::auth::refresh_access_token`].

use std::sync::Arc;

use crate::secrets::SecretsStore;
use crate::tools::mcp::client_tool::ironclaw_refresh_fn;
use crate::tools::mcp::config::McpServerConfig;
use crate::tools::mcp::{McpClient, McpProcessManager, McpSessionManager};

pub use dasclaw_mcp::factory::McpFactoryError;

/// Create an `McpClient` from a server configuration, dispatching on the
/// effective transport type and installing the ironclaw OAuth refresh hook.
pub async fn create_client_from_config(
    server: McpServerConfig,
    session_manager: &Arc<McpSessionManager>,
    process_manager: &Arc<McpProcessManager>,
    secrets: Option<Arc<dyn SecretsStore + Send + Sync>>,
    user_id: &str,
) -> Result<McpClient, McpFactoryError> {
    let client = dasclaw_mcp::factory::create_client_from_config(
        server,
        session_manager,
        process_manager,
        secrets,
        user_id,
    )
    .await?;
    Ok(client.with_refresh_fn(ironclaw_refresh_fn()))
}
