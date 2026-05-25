//! MCP server wiring for the headless CLI (ADR-153 §4.4 sub-step 8).
//!
//! Loads a JSON file listing one or more MCP servers, spawns each one
//! over stdio via [`dasclaw_mcp::StdioMcpTransport`], wraps them as
//! [`dasclaw_mcp::McpClient`] instances and builds a
//! [`dasclaw_mcp::McpToolExecutor`] that the CLI feeds into
//! [`crate::run_with_tools`].
//!
//! # Config shape
//!
//! ```json
//! {
//!   "servers": [
//!     {
//!       "name": "fs",
//!       "command": "npx",
//!       "args": ["@modelcontextprotocol/server-filesystem", "/tmp"],
//!       "env": { "EXTRA": "value" }
//!     }
//!   ]
//! }
//! ```
//!
//! This sub-step only supports stdio servers. HTTP/UDS support stays in
//! `dasclaw_mcp` and is reachable from code; the CLI surface picks the
//! smallest config that's still useful from a single `--mcp-config`
//! flag. Hosted (OAuth-backed) servers are out of scope because the CLI
//! has no persistent secrets store.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dasclaw_mcp::{McpClient, McpToolExecutor, StdioMcpTransport};
use serde::Deserialize;

/// Errors surfaced while loading and starting MCP servers from a config
/// file.
#[derive(Debug, thiserror::Error)]
pub enum McpConfigError {
    /// Could not read the config file.
    #[error("reading MCP config {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// JSON in the config file is invalid.
    #[error("parsing MCP config {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// Could not spawn a stdio MCP server child process.
    #[error("spawning MCP server '{server}': {source}")]
    Spawn {
        server: String,
        #[source]
        source: dasclaw_tool::ToolError,
    },
    /// `tools/list` round-trip failed for at least one server.
    #[error("listing tools from MCP servers: {source}")]
    ListTools {
        #[source]
        source: dasclaw_tool::ToolError,
    },
}

/// Top-level MCP config schema.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct McpConfig {
    /// Ordered list of MCP servers to start. Order is preserved so that,
    /// if two servers ever advertise the same qualified name (same
    /// `name`, same tool), the **last** one wins, matching
    /// [`McpToolExecutor::from_clients`].
    pub servers: Vec<McpServerSpec>,
}

/// One stdio MCP server entry.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct McpServerSpec {
    /// Logical server name. Used both for log lines and as the prefix in
    /// the qualified tool name (`<name>_<tool>`).
    pub name: String,
    /// Executable to spawn (e.g. `npx`, `uvx`, `/usr/bin/my-server`).
    pub command: String,
    /// Arguments passed to `command`. Default empty.
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables for the child process. Default empty.
    /// The parent process's environment is also inherited.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl McpConfig {
    /// Parse a [`McpConfig`] from a JSON string. Exposed for unit tests;
    /// production callers should use [`load_executor`].
    pub fn from_json_str(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Read `path`, spawn one stdio MCP server per entry, and return an
/// [`McpToolExecutor`] aware of every advertised tool.
///
/// This function is async because each server's `tools/list` round-trip
/// is performed up-front so that the agent can advertise the full tool
/// set before the first turn.
pub async fn load_executor(path: &Path) -> Result<McpToolExecutor, McpConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| McpConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let config = McpConfig::from_json_str(&text).map_err(|source| McpConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    spawn_executor(config).await
}

/// Spawn every server in `config` and return the assembled executor.
///
/// Split out from [`load_executor`] so unit tests can exercise the spawn
/// branch with an in-memory config (no file I/O).
async fn spawn_executor(config: McpConfig) -> Result<McpToolExecutor, McpConfigError> {
    let mut clients: Vec<Arc<McpClient>> = Vec::with_capacity(config.servers.len());
    for spec in config.servers {
        let transport =
            StdioMcpTransport::spawn(spec.name.clone(), &spec.command, &spec.args, spec.env)
                .await
                .map_err(|source| McpConfigError::Spawn {
                    server: spec.name.clone(),
                    source,
                })?;
        let client = McpClient::new_with_transport(
            spec.name,
            Arc::new(transport),
            None,
            None,
            "dasclaw-cli",
            None,
        );
        clients.push(Arc::new(client));
    }
    McpToolExecutor::from_clients(clients)
        .await
        .map_err(|source| McpConfigError::ListTools { source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_dasclaw_cli_mcp_c1_parses_minimal_stdio_entry() {
        let cfg =
            McpConfig::from_json_str(r#"{ "servers": [ { "name": "fs", "command": "uvx" } ] }"#)
                .expect("parse");
        assert_eq!(cfg.servers.len(), 1);
        assert_eq!(cfg.servers[0].name, "fs");
        assert_eq!(cfg.servers[0].command, "uvx");
        assert!(cfg.servers[0].args.is_empty());
        assert!(cfg.servers[0].env.is_empty());
    }

    #[test]
    fn req_dasclaw_cli_mcp_c2_parses_args_and_env() {
        let cfg = McpConfig::from_json_str(
            r#"{
                "servers": [
                    {
                        "name": "fs",
                        "command": "npx",
                        "args": ["@modelcontextprotocol/server-filesystem", "/tmp"],
                        "env": { "K": "V" }
                    }
                ]
            }"#,
        )
        .expect("parse");
        assert_eq!(
            cfg.servers[0].args,
            vec![
                "@modelcontextprotocol/server-filesystem".to_string(),
                "/tmp".to_string()
            ]
        );
        assert_eq!(cfg.servers[0].env.get("K").map(String::as_str), Some("V"));
    }

    #[test]
    fn req_dasclaw_cli_mcp_c3_parses_multiple_servers_preserving_order() {
        let cfg = McpConfig::from_json_str(
            r#"{
                "servers": [
                    { "name": "alpha", "command": "a" },
                    { "name": "beta",  "command": "b" }
                ]
            }"#,
        )
        .expect("parse");
        let names: Vec<_> = cfg.servers.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn req_dasclaw_cli_mcp_c4_rejects_missing_required_field() {
        // `command` is required; serde should reject this.
        let err = McpConfig::from_json_str(r#"{ "servers": [ { "name": "x" } ] }"#)
            .expect_err("must reject");
        assert!(
            err.to_string().contains("command"),
            "error should mention missing field `command`, got: {err}"
        );
    }

    #[tokio::test]
    async fn req_dasclaw_cli_mcp_c5_load_executor_surfaces_read_error() {
        let missing = std::path::PathBuf::from("/definitely/does/not/exist.json");
        match load_executor(&missing).await {
            Err(McpConfigError::Read { path, .. }) => assert_eq!(path, missing),
            Err(other) => panic!("expected Read error, got {other:?}"),
            Ok(_) => panic!("expected Read error, got Ok"),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_cli_mcp_c6_load_executor_surfaces_parse_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"{ not json").expect("write");
        match load_executor(&path).await {
            Err(McpConfigError::Parse { .. }) => {}
            Err(other) => panic!("expected Parse error, got {other:?}"),
            Ok(_) => panic!("expected Parse error, got Ok"),
        }
    }

    #[tokio::test]
    async fn req_dasclaw_cli_mcp_c7_spawn_executor_surfaces_spawn_failure() {
        let cfg = McpConfig {
            servers: vec![McpServerSpec {
                name: "ghost".to_string(),
                command: "/definitely/not/a/binary".to_string(),
                args: vec![],
                env: BTreeMap::new(),
            }],
        };
        match spawn_executor(cfg).await {
            Err(McpConfigError::Spawn { server, .. }) => assert_eq!(server, "ghost"),
            Err(other) => panic!("expected Spawn error, got {other:?}"),
            Ok(_) => panic!("expected Spawn error, got Ok"),
        }
    }
}
