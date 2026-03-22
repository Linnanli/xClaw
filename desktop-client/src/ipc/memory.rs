//! 记忆/工作空间 Tauri Commands。
//!
//! 直接调用 `Workspace` 管理记忆文件。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::EngineState;

/// 记忆条目（目录列表用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub content_preview: Option<String>,
}

/// 记忆文档内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDocument {
    pub path: String,
    pub content: String,
    pub updated_at: Option<String>,
}

/// 搜索结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    pub path: String,
    pub content: String,
    pub score: f32,
}

/// 列出记忆目录。
#[tauri::command]
pub async fn ic_memory_list(
    state: State<'_, EngineState>,
    path: Option<String>,
) -> Result<Vec<MemoryEntry>, String> {
    let state = state.get()?;
    let ws = state
        .workspace
        .as_ref()
        .ok_or("Workspace not available")?;

    let dir = path.as_deref().unwrap_or("/");
    let entries = ws
        .list(dir)
        .await
        .map_err(|e| format!("Failed to list memory: {}", e))?;

    Ok(entries
        .into_iter()
        .map(|e| MemoryEntry {
            name: e.name().to_string(),
            path: e.path.clone(),
            is_directory: e.is_directory,
            content_preview: e.content_preview.clone(),
        })
        .collect())
}

/// 读取记忆文档。
#[tauri::command]
pub async fn ic_memory_read(
    state: State<'_, EngineState>,
    path: String,
) -> Result<MemoryDocument, String> {
    let state = state.get()?;
    let ws = state
        .workspace
        .as_ref()
        .ok_or("Workspace not available")?;

    let doc = ws
        .read(&path)
        .await
        .map_err(|e| format!("Failed to read memory: {}", e))?;

    Ok(MemoryDocument {
        path,
        content: doc.content,
        updated_at: Some(doc.updated_at.to_rfc3339()),
    })
}

/// 写入记忆文档。
#[tauri::command]
pub async fn ic_memory_write(
    state: State<'_, EngineState>,
    path: String,
    content: String,
) -> Result<(), String> {
    let state = state.get()?;
    let ws = state
        .workspace
        .as_ref()
        .ok_or("Workspace not available")?;

    ws.write(&path, &content)
        .await
        .map_err(|e| format!("Failed to write memory: {}", e))?;

    tracing::debug!(path = %path, "Memory written");
    Ok(())
}

/// 删除记忆文档。
#[tauri::command]
pub async fn ic_memory_delete(
    state: State<'_, EngineState>,
    path: String,
) -> Result<(), String> {
    let state = state.get()?;
    let ws = state
        .workspace
        .as_ref()
        .ok_or("Workspace not available")?;

    ws.delete(&path)
        .await
        .map_err(|e| format!("Failed to delete memory: {}", e))?;

    tracing::debug!(path = %path, "Memory deleted");
    Ok(())
}

/// 搜索记忆。
#[tauri::command]
pub async fn ic_memory_search(
    state: State<'_, EngineState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<MemorySearchResult>, String> {
    let state = state.get()?;
    let ws = state
        .workspace
        .as_ref()
        .ok_or("Workspace not available")?;

    let results = ws
        .search(&query, limit.unwrap_or(10))
        .await
        .map_err(|e| format!("Failed to search memory: {}", e))?;

    Ok(results
        .into_iter()
        .map(|r| MemorySearchResult {
            path: r.document_path,
            content: r.content,
            score: r.score,
        })
        .collect())
}
