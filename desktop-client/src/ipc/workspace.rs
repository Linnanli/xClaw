//! 工作区状态 Tauri Commands。
//!
//! 提供 Git 状态、工作区根目录、活跃服务器列表等信息，
//! 用于 WorkspaceTab 面板展示。

use serde::Serialize;
use tauri::State;

use crate::state::EngineState;

/// 活跃的 LSP/MCP 服务器。
#[derive(Debug, Clone, Serialize)]
pub struct ActiveServer {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
}

/// 获取当前工作目录的 git status 输出。
#[tauri::command]
pub async fn ic_workspace_git_status(
    state: State<'_, EngineState>,
) -> Result<String, String> {
    let _state = state.get()?;

    let output = tokio::process::Command::new("git")
        .args(["status"])
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git status failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 返回当前工作目录路径。
#[tauri::command]
pub async fn ic_workspace_root(
    state: State<'_, EngineState>,
) -> Result<String, String> {
    let _state = state.get()?;

    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Cannot determine working directory: {e}"))
}

/// 列出当前活跃的 LSP 和 MCP 服务器。
#[tauri::command]
pub async fn ic_active_servers(
    state: State<'_, EngineState>,
) -> Result<Vec<ActiveServer>, String> {
    let state = state.get()?;
    let mut servers = Vec::new();

    // MCP servers from extension manager
    if let Some(ref ext_mgr) = state.extension_manager {
        if let Ok(extensions) = ext_mgr
            .list(
                Some(ironclaw::extensions::ExtensionKind::McpServer),
                false,
                &state.scope_id,
            )
            .await
        {
            for ext in extensions {
                if ext.active {
                    servers.push(ActiveServer {
                        name: ext.name,
                        server_type: "mcp".to_string(),
                    });
                }
            }
        }
    }

    Ok(servers)
}
