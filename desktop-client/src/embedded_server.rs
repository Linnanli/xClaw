///! Embedded IronClaw Server
///! 
///! This module manages the embedded IronClaw server that runs within the Desktop Client.
///! It starts the server on application launch and stops it on application exit.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Port for the embedded IronClaw server
pub const EMBEDDED_SERVER_PORT: u16 = 38080;

/// Embedded server state
pub struct EmbeddedServer {
    handle: Option<JoinHandle<()>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EmbeddedServer {
    /// Create a new embedded server instance
    pub fn new() -> Self {
        Self {
            handle: None,
            shutdown_tx: None,
        }
    }

    /// Start the embedded IronClaw server
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting embedded IronClaw server on port {}", EMBEDDED_SERVER_PORT);

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Set environment variables for IronClaw
        std::env::set_var("SERVER_PORT", EMBEDDED_SERVER_PORT.to_string());
        std::env::set_var("SERVER_HOST", "127.0.0.1");

        // Spawn IronClaw server in a separate task
        let handle = tokio::spawn(async move {
            // Load .env files
            let _ = dotenvy::dotenv();
            ironclaw::bootstrap::load_ironclaw_env();

            // Build IronClaw application
            match build_ironclaw_app().await {
                Ok(()) => {
                    info!("IronClaw server started successfully");
                }
                Err(e) => {
                    error!("Failed to start IronClaw server: {}", e);
                }
            }

            // Wait for shutdown signal
            let _ = shutdown_rx.await;
            info!("IronClaw server shutting down");
        });

        self.handle = Some(handle);

        // Wait for server to be ready
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify server is running
        match check_server_health().await {
            Ok(()) => {
                info!("Embedded IronClaw server is healthy");
                Ok(())
            }
            Err(e) => {
                error!("Embedded IronClaw server health check failed: {}", e);
                Err(e)
            }
        }
    }

    /// Stop the embedded IronClaw server
    pub async fn stop(&mut self) {
        info!("Stopping embedded IronClaw server");

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for server to stop
        if let Some(handle) = self.handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        info!("Embedded IronClaw server stopped");
    }
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        // Note: This is a synchronous drop, so we can't await
        // The shutdown will be handled by the runtime
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Build and run the IronClaw application
async fn build_ironclaw_app() -> Result<(), Box<dyn std::error::Error>> {
    // 简化方案：直接使用 IronClaw 的 CLI 启动
    // 这样可以避免重复实现复杂的启动逻辑
    
    // 设置环境变量
    std::env::set_var("SERVER_PORT", EMBEDDED_SERVER_PORT.to_string());
    std::env::set_var("SERVER_HOST", "127.0.0.1");
    std::env::set_var("GATEWAY_PORT", EMBEDDED_SERVER_PORT.to_string());
    std::env::set_var("GATEWAY_HOST", "127.0.0.1");
    
    // 使用 tokio::process::Command 启动 IronClaw CLI
    let mut child = tokio::process::Command::new("cargo")
        .args(&[
            "run",
            "--manifest-path",
            "../ironclaw/Cargo.toml",
            "--",
            "run",
            "--no-onboard",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    
    info!("IronClaw server process started");
    
    // 等待进程结束
    let status = child.wait().await?;
    
    if !status.success() {
        error!("IronClaw server exited with status: {}", status);
        return Err("IronClaw server failed".into());
    }
    
    Ok(())
}

/// Check if the embedded server is healthy
async fn check_server_health() -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("http://127.0.0.1:{}/api/health", EMBEDDED_SERVER_PORT);
    
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Health check failed with status: {}", response.status()).into())
    }
}

/// Global embedded server instance
static EMBEDDED_SERVER: Mutex<Option<EmbeddedServer>> = Mutex::const_new(None);

/// Start the global embedded server
pub async fn start_global_server() -> Result<(), Box<dyn std::error::Error>> {
    let mut server_guard = EMBEDDED_SERVER.lock().await;
    
    if server_guard.is_some() {
        warn!("Embedded server is already running");
        return Ok(());
    }

    let mut server = EmbeddedServer::new();
    server.start().await?;
    *server_guard = Some(server);

    Ok(())
}

/// Stop the global embedded server
pub async fn stop_global_server() {
    let mut server_guard = EMBEDDED_SERVER.lock().await;
    
    if let Some(mut server) = server_guard.take() {
        server.stop().await;
    }
}

/// Get the embedded server URL
pub fn get_server_url() -> String {
    format!("http://127.0.0.1:{}", EMBEDDED_SERVER_PORT)
}
