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
pub async fn ic_workspace_git_status(state: State<'_, EngineState>) -> Result<String, String> {
    let _state = state.get()?;

    let output = tokio::process::Command::new("git")
        .args(["status"])
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {e}"))?;

    git_status_result(output.status.success(), &output.stdout, &output.stderr)
}

pub(crate) fn git_status_result(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<String, String> {
    if !success {
        let stderr = String::from_utf8_lossy(stderr);
        return Err(format!("git status failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(stdout).into_owned())
}

/// 返回当前工作目录路径。
#[tauri::command]
pub async fn ic_workspace_root(state: State<'_, EngineState>) -> Result<String, String> {
    let _state = state.get()?;

    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("Cannot determine working directory: {e}"))
}

/// 列出当前活跃的 LSP 和 MCP 服务器。
#[tauri::command]
pub async fn ic_active_servers(state: State<'_, EngineState>) -> Result<Vec<ActiveServer>, String> {
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

    let canonical = crate::workspace_dir::validate_import_path(&path)
        .map_err(|e| format!("Invalid workspace path: {e}"))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    db.update_conversation_metadata_field(tid, "workspace_root", &serde_json::json!(canonical_str))
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

    Ok(meta.and_then(|m| {
        m.get("workspace_root")
            .and_then(|v| v.as_str().map(String::from))
    }))
}

/// 列出 `~/.ironclaw/projects/` 下所有沙箱工作区。
#[tauri::command]
pub async fn ic_list_sandbox_workspaces() -> Result<Vec<SandboxWorkspace>, String> {
    let base = crate::workspace_dir::projects_base();
    if !tokio::fs::try_exists(&base).await.unwrap_or(false) {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&base)
        .await
        .map_err(|e| format!("Failed to read projects directory: {e}"))?;

    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e: std::io::Error| e.to_string())?
    {
        let ft = entry
            .file_type()
            .await
            .map_err(|e: std::io::Error| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::{git_status_result, ActiveServer, SandboxWorkspace};

    #[test]
    fn req_workspace_git_status_returns_stdout_on_success() {
        let stdout = b"On branch xClaw\nnothing to commit, working tree clean\n";

        let result = git_status_result(true, stdout, b"").expect("status should succeed");

        assert!(result.contains("On branch xClaw"));
        assert!(result.contains("working tree clean"));
    }

    #[test]
    fn test_workspace_failure_git_status_surfaces_stderr() {
        let err = git_status_result(false, b"", b"fatal: not a git repository")
            .expect_err("status should fail");

        assert!(err.contains("git status failed"));
        assert!(err.contains("not a git repository"));
    }

    #[test]
    fn req_workspace_active_server_contract_renames_type_field() {
        let server = ActiveServer {
            name: "filesystem".to_string(),
            server_type: "mcp".to_string(),
        };

        let json = serde_json::to_value(&server).expect("serialize active server");

        assert_eq!(json["name"], "filesystem");
        assert_eq!(json["type"], "mcp");
        assert!(json.get("server_type").is_none());
    }

    #[test]
    fn req_workspace_sandbox_workspace_contract_matches_frontend() {
        let workspace = SandboxWorkspace {
            name: "thread-123".to_string(),
            path: "/tmp/thread-123".to_string(),
        };

        let json = serde_json::to_value(&workspace).expect("serialize workspace");

        assert_eq!(json["name"], "thread-123");
        assert_eq!(json["path"], "/tmp/thread-123");
    }
}
