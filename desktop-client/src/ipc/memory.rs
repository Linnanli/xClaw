//! 记忆/工作空间 Tauri Commands。
//!
//! 直接调用 `Workspace` 管理记忆文件。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::{AppState, EngineState};

const DEFAULT_MEMORY_SEARCH_LIMIT: usize = 10;

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

pub(crate) fn memory_list_path(path: Option<&str>) -> &str {
    path.unwrap_or("/")
}

pub(crate) fn memory_search_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_MEMORY_SEARCH_LIMIT)
}

pub(crate) fn memory_display_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

pub(crate) fn workspace_entry_to_memory_entry(
    entry: ironclaw::workspace::WorkspaceEntry,
) -> MemoryEntry {
    MemoryEntry {
        name: entry.name().to_string(),
        path: memory_display_path(&entry.path),
        is_directory: entry.is_directory,
        content_preview: entry.content_preview,
    }
}

pub(crate) fn workspace_document_to_memory_document(
    path: String,
    document: ironclaw::workspace::MemoryDocument,
) -> MemoryDocument {
    MemoryDocument {
        path,
        content: document.content,
        updated_at: Some(document.updated_at.to_rfc3339()),
    }
}

pub(crate) fn workspace_search_result_to_memory_search_result(
    result: ironclaw::workspace::SearchResult,
) -> MemorySearchResult {
    MemorySearchResult {
        path: memory_display_path(&result.document_path),
        content: result.content,
        score: result.score,
    }
}

fn app_workspace(state: &AppState) -> Result<&ironclaw::workspace::Workspace, String> {
    state
        .workspace
        .as_deref()
        .ok_or("Workspace not available".to_string())
}

pub(crate) async fn memory_list_from_workspace(
    ws: &ironclaw::workspace::Workspace,
    path: Option<&str>,
) -> Result<Vec<MemoryEntry>, String> {
    let dir = memory_list_path(path);
    let entries = ws
        .list(dir)
        .await
        .map_err(|e| format!("Failed to list memory: {}", e))?;

    Ok(entries
        .into_iter()
        .map(workspace_entry_to_memory_entry)
        .collect())
}

pub(crate) async fn memory_read_from_workspace(
    ws: &ironclaw::workspace::Workspace,
    path: String,
) -> Result<MemoryDocument, String> {
    let doc = ws
        .read(&path)
        .await
        .map_err(|e| format!("Failed to read memory: {}", e))?;

    Ok(workspace_document_to_memory_document(path, doc))
}

pub(crate) async fn memory_write_from_workspace(
    ws: &ironclaw::workspace::Workspace,
    path: &str,
    content: &str,
) -> Result<(), String> {
    ws.write(path, content)
        .await
        .map_err(|e| format!("Failed to write memory: {}", e))?;

    tracing::debug!(path = %path, "Memory written");
    Ok(())
}

pub(crate) async fn memory_delete_from_workspace(
    ws: &ironclaw::workspace::Workspace,
    path: &str,
) -> Result<(), String> {
    ws.delete(path)
        .await
        .map_err(|e| format!("Failed to delete memory: {}", e))?;

    tracing::debug!(path = %path, "Memory deleted");
    Ok(())
}

pub(crate) async fn memory_search_from_workspace(
    ws: &ironclaw::workspace::Workspace,
    query: &str,
    limit: Option<usize>,
) -> Result<Vec<MemorySearchResult>, String> {
    let results = ws
        .search(query, memory_search_limit(limit))
        .await
        .map_err(|e| format!("Failed to search memory: {}", e))?;

    Ok(results
        .into_iter()
        .map(workspace_search_result_to_memory_search_result)
        .collect())
}

/// 列出记忆目录。
#[tauri::command]
pub async fn ic_memory_list(
    state: State<'_, EngineState>,
    path: Option<String>,
) -> Result<Vec<MemoryEntry>, String> {
    let state = state.get()?;
    memory_list_from_workspace(app_workspace(state)?, path.as_deref()).await
}

/// 读取记忆文档。
#[tauri::command]
pub async fn ic_memory_read(
    state: State<'_, EngineState>,
    path: String,
) -> Result<MemoryDocument, String> {
    let state = state.get()?;
    memory_read_from_workspace(app_workspace(state)?, path).await
}

/// 写入记忆文档。
#[tauri::command]
pub async fn ic_memory_write(
    state: State<'_, EngineState>,
    path: String,
    content: String,
) -> Result<(), String> {
    let state = state.get()?;
    memory_write_from_workspace(app_workspace(state)?, &path, &content).await
}

/// 删除记忆文档。
#[tauri::command]
pub async fn ic_memory_delete(state: State<'_, EngineState>, path: String) -> Result<(), String> {
    let state = state.get()?;
    memory_delete_from_workspace(app_workspace(state)?, &path).await
}

/// 搜索记忆。
#[tauri::command]
pub async fn ic_memory_search(
    state: State<'_, EngineState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<MemorySearchResult>, String> {
    let state = state.get()?;
    memory_search_from_workspace(app_workspace(state)?, &query, limit).await
}
