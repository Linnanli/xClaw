//! 扩展管理 Tauri Commands。
//!
//! 直接调用 `ExtensionManager` 管理扩展（MCP、WASM 等）。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::EngineState;

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
    state: State<'_, EngineState>,
    include_available: Option<bool>,
) -> Result<Vec<ExtensionInfo>, String> {
    let state = state.get()?;
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
    state: State<'_, EngineState>,
    name: String,
    url: Option<String>,
) -> Result<String, String> {
    let state = state.get()?;
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
    state: State<'_, EngineState>,
    name: String,
) -> Result<String, String> {
    let state = state.get()?;
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

// ─── 扩展配置（Setup）数据类型 ────────────────────────────────────

/// 扩展配置字段信息（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSetupField {
    pub name: String,
    pub prompt: String,
    pub optional: bool,
    pub provided: bool,
    pub auto_generate: bool,
}

/// 扩展配置 Schema 响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSetupResponse {
    pub name: String,
    pub kind: String,
    pub secrets: Vec<ExtensionSetupField>,
}

/// 扩展配置提交响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSetupSubmitResponse {
    pub success: bool,
    pub message: String,
    pub activated: bool,
    pub auth_url: Option<String>,
}

// ─── 扩展配置 Tauri Commands ─────────────────────────────────────

/// 获取扩展配置 Schema。
///
/// 返回扩展所需的配置字段列表（API Key、Token 等），
/// 前端据此渲染配置表单。
#[tauri::command]
pub async fn ic_extension_setup(
    state: State<'_, EngineState>,
    name: String,
) -> Result<ExtensionSetupResponse, String> {
    let state = state.get()?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let secrets = ext_mgr
        .get_setup_schema(&name)
        .await
        .map_err(|e| format!("Failed to get setup schema: {}", e))?;

    let kind = ext_mgr
        .list(None, false)
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.name == name))
        .map(|e| e.kind.to_string())
        .unwrap_or_default();

    let fields: Vec<ExtensionSetupField> = secrets
        .into_iter()
        .map(|s| ExtensionSetupField {
            name: s.name,
            prompt: s.prompt,
            optional: s.optional,
            provided: s.provided,
            auto_generate: s.auto_generate,
        })
        .collect();

    tracing::debug!(extension = %name, fields = fields.len(), "Extension setup schema loaded");

    Ok(ExtensionSetupResponse {
        name,
        kind,
        secrets: fields,
    })
}

/// 提交扩展配置。
///
/// 将用户填写的 secrets 提交给 ExtensionManager 进行配置和激活。
#[tauri::command]
pub async fn ic_extension_setup_submit(
    state: State<'_, EngineState>,
    name: String,
    secrets: std::collections::HashMap<String, String>,
) -> Result<ExtensionSetupSubmitResponse, String> {
    let state = state.get()?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let result = ext_mgr
        .configure(&name, &secrets)
        .await
        .map_err(|e| format!("Failed to configure extension: {}", e))?;

    tracing::info!(
        extension = %name,
        activated = result.activated,
        "Extension configured"
    );

    Ok(ExtensionSetupSubmitResponse {
        success: result.activated || result.verification.is_some(),
        message: result.message,
        activated: result.activated,
        auth_url: result.auth_url,
    })
}

/// 搜索扩展。
#[tauri::command]
pub async fn ic_search_extensions(
    state: State<'_, EngineState>,
    query: String,
) -> Result<Vec<ExtensionInfo>, String> {
    let state = state.get()?;
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
