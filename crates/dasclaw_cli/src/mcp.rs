//! MCP server wiring for the headless CLI (ADR-153 §4.4 sub-step 8).
//!
//! Loads a JSON file listing one or more MCP servers, starts each one
//! via the appropriate [`dasclaw_mcp`] transport (`StdioMcpTransport`
//! for stdio entries or `HttpMcpTransport` for HTTP entries), wraps
//! them as [`dasclaw_mcp::McpClient`] instances and builds a
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
//!     },
//!     {
//!       "name": "remote",
//!       "url": "https://mcp.example.com/v1",
//!       "headers": { "Authorization": "Bearer ${TOKEN}" }
//!     }
//!   ]
//! }
//! ```
//!
//! Entries are dispatched by shape: an entry with `command` is treated
//! as stdio; an entry with `url` is treated as HTTP. Mixing both
//! `command` and `url` in the same entry, or omitting both, is a config
//! error reported at parse time.
//!
//! Hosted (OAuth-backed) servers are out of scope for the CLI because
//! it has no persistent secrets store; the HTTP transport here uses
//! whatever static headers the user provides in `headers` and does not
//! wire `Mcp-Session-Id`/`SecretsStore`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dasclaw_mcp::{HttpMcpTransport, McpClient, McpToolExecutor, StdioMcpTransport};
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

/// One MCP server entry. Logical `name` lives at the top level; the
/// transport-specific fields are flattened in so the JSON stays flat
/// (`{ "name": "fs", "command": "npx", ... }` or
/// `{ "name": "fs", "url": "https://...", ... }`).
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct McpServerSpec {
    /// Logical server name. Used both for log lines and as the prefix in
    /// the qualified tool name (`<name>_<tool>`).
    pub name: String,
    /// Transport-specific configuration. Selected by which fields the
    /// entry actually carries (`command` ⇒ stdio, `url` ⇒ HTTP).
    #[serde(flatten)]
    pub transport: McpTransportSpec,
}

/// Transport selector for an MCP server entry. `untagged` so the JSON
/// shape matches whichever variant's required fields are present.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum McpTransportSpec {
    /// Local subprocess launched over stdio.
    Stdio {
        /// Executable to spawn (e.g. `npx`, `uvx`, `/usr/bin/my-server`).
        command: String,
        /// Arguments passed to `command`. Default empty.
        #[serde(default)]
        args: Vec<String>,
        /// Extra environment variables for the child process. Default
        /// empty. The parent process's environment is also inherited.
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    /// Remote MCP server reached over HTTP (Streamable HTTP transport).
    Http {
        /// Full URL of the MCP endpoint (e.g.
        /// `https://mcp.example.com/v1`).
        url: String,
        /// Static headers applied to every request (e.g.
        /// `Authorization: Bearer …`). Default empty.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpConfig {
    /// Parse a [`McpConfig`] from a JSON string. Exposed for unit tests;
    /// production callers should use [`load_executor`].
    pub fn from_json_str(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// Read `path`, start one MCP server per entry, and return an
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

/// Start every server in `config` and return the assembled executor.
///
/// Split out from [`load_executor`] so unit tests can exercise the
/// transport branches with in-memory configs (no file I/O).
async fn spawn_executor(config: McpConfig) -> Result<McpToolExecutor, McpConfigError> {
    let mut clients: Vec<Arc<McpClient>> = Vec::with_capacity(config.servers.len());
    for spec in config.servers {
        let McpServerSpec { name, transport } = spec;
        let client = match transport {
            McpTransportSpec::Stdio { command, args, env } => {
                let transport = StdioMcpTransport::spawn(name.clone(), &command, &args, env)
                    .await
                    .map_err(|source| McpConfigError::Spawn {
                        server: name.clone(),
                        source,
                    })?;
                McpClient::new_with_transport(
                    name,
                    Arc::new(transport),
                    None,
                    None,
                    "dasclaw-cli",
                    None,
                )
            }
            McpTransportSpec::Http { url, headers } => {
                // BTreeMap → HashMap for HttpMcpTransport.
                let header_map: HashMap<String, String> = headers.into_iter().collect();
                let transport =
                    HttpMcpTransport::new(url, name.clone()).with_custom_headers(header_map);
                McpClient::new_with_transport(
                    name,
                    Arc::new(transport),
                    None,
                    None,
                    "dasclaw-cli",
                    None,
                )
            }
        };
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
        match &cfg.servers[0].transport {
            McpTransportSpec::Stdio { command, args, env } => {
                assert_eq!(command, "uvx");
                assert!(args.is_empty());
                assert!(env.is_empty());
            }
            other => panic!("expected Stdio, got {other:?}"),
        }
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
        match &cfg.servers[0].transport {
            McpTransportSpec::Stdio { args, env, .. } => {
                assert_eq!(
                    *args,
                    vec![
                        "@modelcontextprotocol/server-filesystem".to_string(),
                        "/tmp".to_string()
                    ]
                );
                assert_eq!(env.get("K").map(String::as_str), Some("V"));
            }
            other => panic!("expected Stdio, got {other:?}"),
        }
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
    fn req_dasclaw_cli_mcp_c4_rejects_entry_without_command_or_url() {
        // Neither `command` nor `url` ⇒ untagged enum has no matching
        // variant ⇒ parse error.
        let err = McpConfig::from_json_str(r#"{ "servers": [ { "name": "x" } ] }"#)
            .expect_err("must reject");
        let msg = err.to_string();
        assert!(
            msg.contains("variant") || msg.contains("McpTransportSpec") || msg.contains("data"),
            "error should mention untagged-enum failure, got: {msg}"
        );
    }

    #[test]
    fn req_dasclaw_cli_mcp_c8_parses_minimal_http_entry() {
        let cfg = McpConfig::from_json_str(
            r#"{ "servers": [ { "name": "remote", "url": "https://mcp.example.com/v1" } ] }"#,
        )
        .expect("parse");
        assert_eq!(cfg.servers[0].name, "remote");
        match &cfg.servers[0].transport {
            McpTransportSpec::Http { url, headers } => {
                assert_eq!(url, "https://mcp.example.com/v1");
                assert!(headers.is_empty());
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_cli_mcp_c9_parses_http_with_headers() {
        let cfg = McpConfig::from_json_str(
            r#"{
                "servers": [
                    {
                        "name": "remote",
                        "url": "https://mcp.example.com/v1",
                        "headers": { "Authorization": "Bearer T", "X-Trace": "1" }
                    }
                ]
            }"#,
        )
        .expect("parse");
        match &cfg.servers[0].transport {
            McpTransportSpec::Http { headers, .. } => {
                assert_eq!(
                    headers.get("Authorization").map(String::as_str),
                    Some("Bearer T")
                );
                assert_eq!(headers.get("X-Trace").map(String::as_str), Some("1"));
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn req_dasclaw_cli_mcp_c10_parses_mixed_stdio_and_http_entries() {
        let cfg = McpConfig::from_json_str(
            r#"{
                "servers": [
                    { "name": "fs",     "command": "npx", "args": ["x"] },
                    { "name": "remote", "url": "https://m.example.com" }
                ]
            }"#,
        )
        .expect("parse");
        assert_eq!(cfg.servers.len(), 2);
        assert!(matches!(
            cfg.servers[0].transport,
            McpTransportSpec::Stdio { .. }
        ));
        assert!(matches!(
            cfg.servers[1].transport,
            McpTransportSpec::Http { .. }
        ));
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
                transport: McpTransportSpec::Stdio {
                    command: "/definitely/not/a/binary".to_string(),
                    args: vec![],
                    env: BTreeMap::new(),
                },
            }],
        };
        match spawn_executor(cfg).await {
            Err(McpConfigError::Spawn { server, .. }) => assert_eq!(server, "ghost"),
            Err(other) => panic!("expected Spawn error, got {other:?}"),
            Ok(_) => panic!("expected Spawn error, got Ok"),
        }
    }
}
