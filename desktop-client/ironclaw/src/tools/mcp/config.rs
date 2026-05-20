//! MCP server configuration — ironclaw host shim.
//!
//! The host-agnostic surface (types, validation, explicit-path JSON
//! load/save, `is_localhost_url`, `EffectiveTransport`) lives in
//! [`dasclaw_mcp::config`]; this module only adds the ironclaw-specific
//! default-path wrappers (rooted at `dasclaw_base_dir()/mcp-servers.json`)
//! and the database-backed wrappers that go through `crate::db::Database`.
//!
//! Migrated in F3.2 phase 2 PR 5a (#641).

use std::path::PathBuf;

pub use dasclaw_mcp::config::{
    ConfigError, EffectiveTransport, McpServerConfig, McpServersFile, McpTransportConfig,
    OAuthConfig, is_localhost_url, load_mcp_servers_from, save_mcp_servers_to,
};

use crate::bootstrap::dasclaw_base_dir;

/// Get the default MCP servers configuration path.
pub fn default_config_path() -> PathBuf {
    dasclaw_base_dir().join("mcp-servers.json")
}

/// Load MCP server configurations from the default location.
pub async fn load_mcp_servers() -> Result<McpServersFile, ConfigError> {
    load_mcp_servers_from(default_config_path()).await
}

/// Save MCP server configurations to the default location.
pub async fn save_mcp_servers(config: &McpServersFile) -> Result<(), ConfigError> {
    save_mcp_servers_to(config, default_config_path()).await
}

/// Add a new MCP server configuration.
pub async fn add_mcp_server(config: McpServerConfig) -> Result<(), ConfigError> {
    config.validate()?;

    let mut servers = load_mcp_servers().await?;
    servers.upsert(config);
    save_mcp_servers(&servers).await?;

    Ok(())
}

/// Remove an MCP server by name.
pub async fn remove_mcp_server(name: &str) -> Result<(), ConfigError> {
    let mut servers = load_mcp_servers().await?;

    if !servers.remove(name) {
        return Err(ConfigError::ServerNotFound {
            name: name.to_string(),
        });
    }

    save_mcp_servers(&servers).await?;

    Ok(())
}

/// Get a specific MCP server configuration.
pub async fn get_mcp_server(name: &str) -> Result<McpServerConfig, ConfigError> {
    let servers = load_mcp_servers().await?;

    servers
        .get(name)
        .cloned()
        .ok_or_else(|| ConfigError::ServerNotFound {
            name: name.to_string(),
        })
}

// ==================== Database-backed MCP server config ====================

/// Load MCP server configurations from the database settings table.
///
/// Falls back to the disk file if DB has no entry.
pub async fn load_mcp_servers_from_db(
    store: &dyn crate::db::Database,
    user_id: &str,
) -> Result<McpServersFile, ConfigError> {
    match store.get_setting(user_id, "mcp_servers").await {
        Ok(Some(value)) => {
            let config: McpServersFile = serde_json::from_value(value)?;
            // Validate every server on load so corrupted DB configs are caught early
            for server in &config.servers {
                server.validate().map_err(|e| ConfigError::InvalidConfig {
                    reason: format!("Server '{}': {}", server.name, e),
                })?;
            }
            Ok(config)
        }
        Ok(None) => {
            // No entry in DB, fall back to disk
            load_mcp_servers().await
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load MCP servers from DB: {}, falling back to disk",
                e
            );
            load_mcp_servers().await
        }
    }
}

/// Save MCP server configurations to the database settings table.
pub async fn save_mcp_servers_to_db(
    store: &dyn crate::db::Database,
    user_id: &str,
    config: &McpServersFile,
) -> Result<(), ConfigError> {
    let value = serde_json::to_value(config)?;
    store
        .set_setting(user_id, "mcp_servers", &value)
        .await
        .map_err(std::io::Error::other)?;
    Ok(())
}

/// Add a new MCP server configuration (DB-backed).
pub async fn add_mcp_server_db(
    store: &dyn crate::db::Database,
    user_id: &str,
    config: McpServerConfig,
) -> Result<(), ConfigError> {
    config.validate()?;

    let mut servers = load_mcp_servers_from_db(store, user_id).await?;
    servers.upsert(config);
    save_mcp_servers_to_db(store, user_id, &servers).await?;

    Ok(())
}

/// Remove an MCP server by name (DB-backed).
pub async fn remove_mcp_server_db(
    store: &dyn crate::db::Database,
    user_id: &str,
    name: &str,
) -> Result<(), ConfigError> {
    let mut servers = load_mcp_servers_from_db(store, user_id).await?;

    if !servers.remove(name) {
        return Err(ConfigError::ServerNotFound {
            name: name.to_string(),
        });
    }

    save_mcp_servers_to_db(store, user_id, &servers).await?;
    Ok(())
}
