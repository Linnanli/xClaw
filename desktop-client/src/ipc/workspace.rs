//! 工作区状态 Tauri Commands。
//!
//! 提供 Git 状态、工作区根目录、活跃服务器列表等信息，
//! 以及工作区导入/查询功能，用于桌面客户端侧边栏。

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

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

/// 为指定线程导入用户工作区。
///
/// 验证路径有效性后将其写入对话 metadata 的 `workspace_root` 字段，
/// 覆盖自动创建的沙箱路径。
#[tauri::command]
pub async fn ic_import_workspace(
    state: State<'_, EngineState>,
    thread_id: String,
    path: String,
) -> Result<String, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let tid: Uuid = thread_id
        .parse()
        .map_err(|_| format!("Invalid thread_id: {thread_id}"))?;

    let canonical = ironclaw::workspace_dir::validate_import_path(&path)
        .map_err(|e| format!("Invalid workspace path: {e}"))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    db.update_conversation_metadata_field(
        tid,
        "workspace_root",
        &serde_json::json!(canonical_str),
    )
    .await
    .map_err(|e| format!("Failed to update workspace: {e}"))?;

    tracing::info!(thread_id = %tid, workspace = %canonical_str, "Workspace imported");
    Ok(canonical_str)
}

/// 查询指定线程的工作区路径。
#[tauri::command]
pub async fn ic_get_thread_workspace(
    state: State<'_, EngineState>,
    thread_id: String,
) -> Result<Option<String>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let tid: Uuid = thread_id
        .parse()
        .map_err(|_| format!("Invalid thread_id: {thread_id}"))?;

    let meta = db
        .get_conversation_metadata(tid)
        .await
        .map_err(|e| format!("Failed to read metadata: {e}"))?;

    Ok(meta
        .and_then(|m| m.get("workspace_root").and_then(|v| v.as_str().map(String::from))))
}

/// 列出 `~/.ironclaw/projects/` 下所有沙箱工作区。
#[tauri::command]
pub async fn ic_list_sandbox_workspaces() -> Result<Vec<SandboxWorkspace>, String> {
    let base = ironclaw::workspace_dir::projects_base();
    if !tokio::fs::try_exists(&base).await.unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&base)
        .await
        .map_err(|e| format!("Failed to read projects directory: {e}"))?;

    while let Some(entry) = dir.next_entry().await.map_err(|e| e.to_string())? {
        let ft = entry.file_type().await.map_err(|e| e.to_string())?;
        if ft.is_dir() {
            entries.push(SandboxWorkspace {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
            });
        }
    }

    Ok(entries)
}

/// 沙箱工作区简要信息。
#[derive(Debug, Clone, Serialize)]
pub struct SandboxWorkspace {
    pub name: String,
    pub path: String,
}
