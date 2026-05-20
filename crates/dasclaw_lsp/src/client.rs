//! LSP JSON-RPC client over stdio.
//!
//! Spawns a language server as a child process and communicates via
//! Content-Length–framed JSON-RPC over stdin/stdout. Modeled after
//! the MCP `StdioMcpTransport` pattern.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

use super::protocol::{LspNotification, LspRequest, LspResponse};
use dasclaw_tool::ToolError;

/// Default timeout for individual LSP requests.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// LSP client that manages communication with a single language server process.
pub struct LspClient {
    server_name: String,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<LspResponse>>>>,
    next_id: AtomicU64,
    reader_handle: Mutex<Option<JoinHandle<()>>>,
    stderr_handle: Mutex<Option<JoinHandle<()>>>,
    child: Arc<Mutex<Child>>,
    /// Whether the initialize handshake has completed.
    initialized: Mutex<bool>,
}

impl LspClient {
    /// Spawn a language server process and create a client.
    pub async fn spawn(
        name: impl Into<String>,
        command: &str,
        args: &[String],
        root_uri: &str,
    ) -> Result<Self, ToolError> {
        let server_name = name.into();

        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.kill_on_drop(true).spawn().map_err(|e| {
            ToolError::ExternalService(format!(
                "[{}] Failed to spawn LSP server '{}': {}",
                server_name, command, e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ToolError::ExternalService(format!("[{}] Failed to capture stdin", server_name))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::ExternalService(format!("[{}] Failed to capture stdout", server_name))
        })?;

        let stderr = child.stderr.take().ok_or_else(|| {
            ToolError::ExternalService(format!("[{}] Failed to capture stderr", server_name))
        })?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<LspResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reader_handle =
            spawn_lsp_reader(BufReader::new(stdout), pending.clone(), server_name.clone());

        let stderr_name = server_name.clone();
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("[{}] stderr: {}", stderr_name, line);
            }
        });

        let client = Self {
            server_name,
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            reader_handle: Mutex::new(Some(reader_handle)),
            stderr_handle: Mutex::new(Some(stderr_handle)),
            child: Arc::new(Mutex::new(child)),
            initialized: Mutex::new(false),
        };

        client.initialize(root_uri).await?;

        Ok(client)
    }

    /// Perform the LSP initialize handshake.
    async fn initialize(&self, root_uri: &str) -> Result<(), ToolError> {
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["plaintext"] },
                    "completion": { "completionItem": {} },
                    "definition": {},
                    "references": {},
                    "documentSymbol": {},
                    "rename": { "prepareSupport": false },
                    "publishDiagnostics": {}
                }
            }
        });

        let _result = self.request("initialize", params).await?;
        self.notify("initialized", None).await?;
        *self.initialized.lock().await = true;
        Ok(())
    }

    /// Send a request and wait for the response.
    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = LspRequest::new(id, method, params);

        let encoded = req.encode().map_err(|e| {
            ToolError::ExecutionFailed(format!("[{}] JSON encode: {}", self.server_name, e))
        })?;

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(&encoded).await.map_err(|e| {
                ToolError::ExternalService(format!(
                    "[{}] Failed to write to stdin: {}",
                    self.server_name, e
                ))
            })?;
            stdin.flush().await.ok();
        }

        let resp = tokio::time::timeout(REQUEST_TIMEOUT, rx)
            .await
            .map_err(|_| ToolError::Timeout(REQUEST_TIMEOUT))?
            .map_err(|_| {
                ToolError::ExternalService(format!(
                    "[{}] Response channel closed",
                    self.server_name
                ))
            })?;

        if let Some(err) = resp.error {
            return Err(ToolError::ExternalService(format!(
                "[{}] {}",
                self.server_name, err
            )));
        }

        Ok(resp.result.unwrap_or(serde_json::Value::Null))
    }

    /// Send a notification (no response expected).
    pub async fn notify(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), ToolError> {
        let notif = LspNotification::new(method, params);
        let encoded = notif.encode().map_err(|e| {
            ToolError::ExecutionFailed(format!("[{}] JSON encode: {}", self.server_name, e))
        })?;

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(&encoded).await.map_err(|e| {
            ToolError::ExternalService(format!(
                "[{}] Failed to write notification: {}",
                self.server_name, e
            ))
        })?;
        stdin.flush().await.ok();
        Ok(())
    }

    /// Notify the server that a file was opened.
    pub async fn did_open(
        &self,
        uri: &str,
        language_id: &str,
        text: &str,
    ) -> Result<(), ToolError> {
        self.notify(
            "textDocument/didOpen",
            Some(serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            })),
        )
        .await
    }

    /// Notify the server that a file was closed.
    pub async fn did_close(&self, uri: &str) -> Result<(), ToolError> {
        self.notify(
            "textDocument/didClose",
            Some(serde_json::json!({
                "textDocument": { "uri": uri }
            })),
        )
        .await
    }

    /// Shut down the LSP server gracefully.
    pub async fn shutdown(&self) {
        // Try graceful shutdown
        if let Ok(resp) = self.request("shutdown", serde_json::Value::Null).await {
            tracing::debug!("[{}] shutdown response: {:?}", self.server_name, resp);
            let _ = self.notify("exit", None).await;
        }

        // Abort background tasks
        if let Some(h) = self.reader_handle.lock().await.take() {
            h.abort();
        }
        if let Some(h) = self.stderr_handle.lock().await.take() {
            h.abort();
        }

        // Kill child if still running
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.kill().await;
        }
    }

    /// Check if the server process is still alive.
    pub async fn is_alive(&self) -> bool {
        if let Ok(mut child) = self.child.try_lock() {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    pub fn name(&self) -> &str {
        &self.server_name
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if let Some(h) = self.reader_handle.get_mut().take() {
            h.abort();
        }
        if let Some(h) = self.stderr_handle.get_mut().take() {
            h.abort();
        }
    }
}

/// Spawn a background task that reads LSP Content-Length–framed messages from
/// stdout and dispatches responses to pending receivers.
fn spawn_lsp_reader(
    mut reader: BufReader<tokio::process::ChildStdout>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<LspResponse>>>>,
    server_name: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match read_lsp_message(&mut reader).await {
                Ok(bytes) => {
                    if let Ok(resp) = serde_json::from_slice::<LspResponse>(&bytes)
                        && let Some(id) = resp.id
                        && let Some(tx) = pending.lock().await.remove(&id)
                    {
                        let _ = tx.send(resp);
                    }
                    // Notifications (no id) are silently dropped; diagnostics
                    // come as server-pushed notifications and are handled
                    // separately if needed.
                }
                Err(e) => {
                    tracing::debug!("[{}] reader stopped: {}", server_name, e);
                    // Wake all pending requests so they don't hang forever.
                    let mut map = pending.lock().await;
                    for (_id, tx) in map.drain() {
                        let _ = tx.send(LspResponse {
                            id: None,
                            result: None,
                            error: Some(super::protocol::LspError {
                                code: -1,
                                message: "Server connection lost".to_string(),
                            }),
                        });
                    }
                    break;
                }
            }
        }
    })
}

/// Read one Content-Length–framed LSP message.
async fn read_lsp_message(
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<Vec<u8>, std::io::Error> {
    // Parse headers until we find Content-Length
    let mut content_length: Option<usize> = None;
    let mut header_buf = String::new();
    loop {
        header_buf.clear();
        let n = reader.read_line(&mut header_buf).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF while reading LSP header",
            ));
        }
        let line = header_buf.trim();
        if line.is_empty() {
            // Empty line separates headers from body
            break;
        }
        if let Some(val) = line.strip_prefix("Content-Length:")
            && let Ok(len) = val.trim().parse::<usize>()
        {
            content_length = Some(len);
        }
    }

    let len = content_length.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Missing Content-Length header",
        )
    })?;

    // Reject absurdly large messages (10 MB).
    if len > 10 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Content-Length too large: {len}"),
        ));
    }

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(body)
}

/// Convert a file path to a file:// URI.
pub fn path_to_uri(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    format!("file://{}", abs.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_uri_absolute() {
        let uri = path_to_uri(Path::new("/home/user/project/src/main.rs"));
        assert_eq!(uri, "file:///home/user/project/src/main.rs");
    }

    #[test]
    fn test_path_to_uri_relative() {
        let uri = path_to_uri(Path::new("src/main.rs"));
        assert!(uri.starts_with("file://"));
        assert!(uri.ends_with("src/main.rs"));
    }
}
