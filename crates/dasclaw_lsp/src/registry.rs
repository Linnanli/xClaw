//! Language-server registry: lazy start, idle reap, admin whitelist.
//!
//! Provides [`LspRegistry`] — the runtime owner of [`crate::LspClient`]
//! instances keyed by server name. Servers spawn on first request for a
//! given language and shut down automatically after [`IDLE_TIMEOUT`] of
//! inactivity.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::client::{self, LspClient};
use crate::server_config::LspServerMapping;
use dasclaw_tool::ToolError;

/// Idle timeout before an LSP server is automatically shut down.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// An active LSP server entry.
struct ServerEntry {
    client: Arc<LspClient>,
    last_used: Instant,
    root_uri: String,
}

/// Registry that manages language server instances.
///
/// Servers are started on first request for a given language and automatically
/// shut down after [`IDLE_TIMEOUT`] of inactivity.
pub struct LspRegistry {
    servers: Mutex<HashMap<String, ServerEntry>>,
    mapping: Mutex<LspServerMapping>,
}

impl LspRegistry {
    /// Create a new registry with default language mappings.
    pub fn new() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
            mapping: Mutex::new(LspServerMapping::defaults()),
        }
    }

    /// Apply an admin whitelist to restrict available servers.
    pub async fn apply_whitelist(&self, allowed: &[String]) {
        self.mapping.lock().await.apply_whitelist(allowed);
    }

    /// Get (or start) the LSP client for a given file path and workspace root.
    pub async fn client_for_file(
        &self,
        file_path: &Path,
        workspace_root: &Path,
    ) -> Result<Arc<LspClient>, ToolError> {
        let mapping = self.mapping.lock().await;
        let config = mapping.config_for_path(file_path).ok_or_else(|| {
            let ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("?");
            ToolError::InvalidParameters(format!("No language server configured for .{ext} files"))
        })?;

        let server_name = config.name.clone();
        let _language_id = config.language_id.clone();
        let command = config.command.clone();
        let args = config.args.clone();
        drop(mapping); // release lock before starting server

        let root_uri = client::path_to_uri(workspace_root);

        let mut servers = self.servers.lock().await;

        // Reuse existing server if alive and same root.
        if let Some(entry) = servers.get_mut(&server_name) {
            if entry.root_uri == root_uri && entry.client.is_alive().await {
                entry.last_used = Instant::now();
                return Ok(Arc::clone(&entry.client));
            }
            // Dead or different root — shut down and restart.
            entry.client.shutdown().await;
            servers.remove(&server_name);
        }

        tracing::info!(
            "Starting LSP server '{}' for workspace {}",
            server_name,
            workspace_root.display()
        );

        let client = Arc::new(LspClient::spawn(&server_name, &command, &args, &root_uri).await?);

        servers.insert(
            server_name.clone(),
            ServerEntry {
                client: Arc::clone(&client),
                last_used: Instant::now(),
                root_uri,
            },
        );

        Ok(client)
    }

    /// Return the language ID for a file path (used by didOpen).
    pub async fn language_id_for(&self, file_path: &Path) -> Option<String> {
        self.mapping
            .lock()
            .await
            .config_for_path(file_path)
            .map(|c| c.language_id.clone())
    }

    /// Shut down servers that have been idle for more than `IDLE_TIMEOUT`.
    pub async fn reap_idle(&self) {
        let now = Instant::now();
        let mut servers = self.servers.lock().await;
        let idle_names: Vec<String> = servers
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_used) > IDLE_TIMEOUT)
            .map(|(name, _)| name.clone())
            .collect();

        for name in idle_names {
            if let Some(entry) = servers.remove(&name) {
                tracing::info!("Shutting down idle LSP server '{}'", name);
                entry.client.shutdown().await;
            }
        }
    }

    /// Shut down all managed servers.
    pub async fn shutdown_all(&self) {
        let mut servers = self.servers.lock().await;
        for (name, entry) in servers.drain() {
            tracing::info!("Shutting down LSP server '{}'", name);
            entry.client.shutdown().await;
        }
    }

    /// List currently running server names.
    pub async fn running_servers(&self) -> Vec<String> {
        self.servers.lock().await.keys().cloned().collect()
    }
}

impl Default for LspRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_default_creates_empty_servers() {
        // Can only test synchronous aspects without a real LSP server.
        let registry = LspRegistry::new();
        // servers map starts empty
        let servers = registry.servers.try_lock().expect("lock");
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_language_id_for_rust() {
        let registry = LspRegistry::new();
        let lang = registry.language_id_for(Path::new("src/main.rs")).await;
        assert_eq!(lang.as_deref(), Some("rust"));
    }

    #[tokio::test]
    async fn test_language_id_for_unknown() {
        let registry = LspRegistry::new();
        let lang = registry.language_id_for(Path::new("photo.jpg")).await;
        assert!(lang.is_none());
    }

    #[tokio::test]
    async fn test_whitelist_restricts_languages() {
        let registry = LspRegistry::new();
        registry.apply_whitelist(&["rust-analyzer".into()]).await;
        assert!(registry.language_id_for(Path::new("a.rs")).await.is_some());
        assert!(registry.language_id_for(Path::new("a.ts")).await.is_none());
    }

    #[tokio::test]
    async fn test_running_servers_empty_initially() {
        let registry = LspRegistry::new();
        assert!(registry.running_servers().await.is_empty());
    }
}
