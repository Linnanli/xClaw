//! 扩展管理 Tauri Commands。
//!
//! 直接调用 `ExtensionManager` 管理扩展（MCP、WASM 等）。

use serde::{Deserialize, Serialize};
use tauri::State;

use super::persistence::persist_disabled_items;
use crate::state::{AppState, EngineState};

const DISABLED_EXTENSIONS_SETTING_KEY: &str = "desktop_disabled_extensions";

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

fn extension_name_exists<'a>(name: &str, mut names: impl Iterator<Item = &'a str>) -> bool {
    names.any(|candidate| candidate == name)
}

fn effective_active(is_runtime_active: bool, is_soft_enabled: bool) -> bool {
    is_runtime_active && is_soft_enabled
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
        .list(None, include_available.unwrap_or(false), &state.owner_id)
        .await
        .map_err(|e| format!("Failed to list extensions: {}", e))?;

    Ok(extensions
        .into_iter()
        .map(|e| ExtensionInfo {
            name: e.name.clone(),
            display_name: e.display_name.clone(),
            kind: format!("{}", e.kind),
            installed: e.installed,
            active: effective_active(e.active, state.extension_enabled(&e.name)),
            authenticated: e.authenticated,
            tools: e.tools.clone(),
        })
        .collect())
}

/// 启用扩展。
#[tauri::command]
pub async fn ic_enable_extension(
    state: State<'_, EngineState>,
    name: String,
) -> Result<(), String> {
    let state = state.get()?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    ensure_extension_installed(state, &name).await?;
    ext_mgr
        .activate(&name, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to enable extension: {}", e))?;
    if let Err(error) = set_extension_enabled_with_persist(state, &name, true).await {
        let _ = soft_deactivate_extension_runtime(state, &name).await;
        return Err(error);
    }
    tracing::info!(extension = %name, "Extension enabled");
    Ok(())
}

/// 禁用扩展（软禁用，仅对 desktop-client 生效）。
#[tauri::command]
pub async fn ic_disable_extension(
    state: State<'_, EngineState>,
    name: String,
) -> Result<(), String> {
    let state = state.get()?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    ensure_extension_installed(state, &name).await?;
    soft_deactivate_extension_runtime(state, &name).await?;
    if let Err(error) = set_extension_enabled_with_persist(state, &name, false).await {
        if let Err(reactivate_error) = ext_mgr.activate(&name, &state.owner_id).await {
            return Err(format!(
                "{}; failed to reactivate extension after rollback: {}",
                error, reactivate_error
            ));
        }
        return Err(error);
    }

    tracing::info!(extension = %name, "Extension disabled");
    Ok(())
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
        .install(&name, url.as_deref(), None, &state.owner_id)
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
        .remove(&name, &state.owner_id)
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
    pub input_type: String,
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
        .get_setup_schema(&name, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to get setup schema: {}", e))?;

    let kind = ext_mgr
        .list(None, false, &state.owner_id)
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.name == name))
        .map(|e| e.kind.to_string())
        .unwrap_or_default();

    let fields: Vec<ExtensionSetupField> = secrets
        .fields
        .into_iter()
        .map(|s| ExtensionSetupField {
            name: s.name,
            prompt: s.prompt,
            optional: s.optional,
            provided: s.provided,
            input_type: format!("{:?}", s.input_type),
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
        .configure(
            &name,
            &secrets,
            &std::collections::HashMap::new(),
            &state.owner_id,
        )
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

async fn ensure_extension_installed(state: &AppState, name: &str) -> Result<(), String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;
    let installed = ext_mgr
        .list(None, false, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to list extensions: {}", e))?;
    if extension_name_exists(name, installed.iter().map(|ext| ext.name.as_str())) {
        return Ok(());
    }
    Err(format!("Extension not installed: {}", name))
}

async fn persist_disabled_extensions(state: &AppState) -> Result<(), String> {
    let disabled = state.disabled_extensions_snapshot()?;
    persist_disabled_items(
        state,
        DISABLED_EXTENSIONS_SETTING_KEY,
        disabled,
        "extensions",
    )
    .await
}

async fn set_extension_enabled_with_persist(
    state: &AppState,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    state.set_extension_enabled(name, enabled)?;
    if let Err(error) = persist_disabled_extensions(state).await {
        let rollback_error = state
            .set_extension_enabled(name, !enabled)
            .err()
            .unwrap_or_default();
        if rollback_error.is_empty() {
            return Err(error);
        }
        return Err(format!(
            "{}; rollback failed: {}",
            error, rollback_error
        ));
    }
    Ok(())
}

async fn soft_deactivate_extension_runtime(state: &AppState, name: &str) -> Result<(), String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let installed = ext_mgr
        .list(None, false, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to list extensions: {}", e))?;

    let Some(extension) = installed.into_iter().find(|ext| ext.name == name) else {
        return Err(format!("Extension not installed: {}", name));
    };

    for tool in extension.tools {
        state.tools.unregister(&tool).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{effective_active, extension_name_exists};

    #[test]
    fn test_extension_name_exists_when_present() {
        let installed = ["github", "slack", "notion"];
        let found = extension_name_exists("slack", installed.iter().copied());
        assert!(found, "expected installed extension to be found");
    }

    #[test]
    fn test_extension_name_exists_when_missing() {
        let installed = ["github", "slack", "notion"];
        let found = extension_name_exists("linear", installed.iter().copied());
        assert!(!found, "expected missing extension to be rejected");
    }

    #[test]
    fn test_effective_active_requires_runtime_and_soft_enable() {
        assert!(effective_active(true, true));
        assert!(!effective_active(true, false));
        assert!(!effective_active(false, true));
        assert!(!effective_active(false, false));
    }
}
