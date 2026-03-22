//! 扩展管理 Tauri Commands。
//!
//! 直接调用 `ExtensionManager` 管理扩展（MCP、WASM 等）。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 扩展信息（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub name: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub installed: bool,
    pub active: bool,
    pub authenticated: bool,
    pub tools: Vec<String>,
}

/// 列出扩展。
#[tauri::command]
pub async fn ic_list_extensions(
    state: State<'_, AppState>,
    include_available: Option<bool>,
) -> Result<Vec<ExtensionInfo>, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let extensions = ext_mgr
        .list(None, include_available.unwrap_or(false))
        .await
        .map_err(|e| format!("Failed to list extensions: {}", e))?;

    Ok(extensions
        .into_iter()
        .map(|e| ExtensionInfo {
            name: e.name.clone(),
            display_name: e.display_name.clone(),
            kind: format!("{}", e.kind),
            installed: e.installed,
            active: e.active,
            authenticated: e.authenticated,
            tools: e.tools.clone(),
        })
        .collect())
}

/// 安装扩展。
#[tauri::command]
pub async fn ic_install_extension(
    state: State<'_, AppState>,
    name: String,
    url: Option<String>,
) -> Result<String, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let result = ext_mgr
        .install(&name, url.as_deref(), None)
        .await
        .map_err(|e| format!("Failed to install extension: {}", e))?;

    tracing::info!(extension = %name, "Extension installed");
    Ok(result.message)
}

/// 卸载扩展。
#[tauri::command]
pub async fn ic_uninstall_extension(
    state: State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let message = ext_mgr
        .remove(&name)
        .await
        .map_err(|e| format!("Failed to uninstall extension: {}", e))?;

    tracing::info!(extension = %name, "Extension uninstalled");
    Ok(message)
}

/// 搜索扩展。
#[tauri::command]
pub async fn ic_search_extensions(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<ExtensionInfo>, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let results = ext_mgr
        .search(&query, true)
        .await
        .map_err(|e| format!("Failed to search extensions: {}", e))?;

    Ok(results
        .into_iter()
        .map(|r| ExtensionInfo {
            name: r.entry.name.clone(),
            display_name: Some(r.entry.display_name.clone()),
            kind: format!("{}", r.entry.kind),
            installed: false,
            active: false,
            authenticated: false,
            tools: Vec::new(),
        })
        .collect())
}
