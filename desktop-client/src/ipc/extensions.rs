//! 扩展管理 Tauri Commands。
//!
//! 直接调用 `ExtensionManager` 管理扩展（MCP、WASM 等）。

use serde::{Deserialize, Serialize};
use tauri::State;

use super::persistence::{load_string_set, persist_disabled_items};
use crate::managed_policy::load_verified_policy_from_store;
use crate::state::{AppState, EngineState};

const DISABLED_EXTENSIONS_SETTING_KEY: &str = "desktop_disabled_extensions";
const MANAGED_ALLOWED_EXTENSIONS_SETTING_KEY: &str = "desktop_managed_allowed_extensions";

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

fn managed_mode_enabled() -> bool {
    std::env::var("MANAGED_MODE")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "true" || normalized == "1"
        })
        .unwrap_or(false)
}

fn validate_extension_install_source(managed_mode: bool, url: Option<&str>) -> Result<(), String> {
    if managed_mode && url.is_some() {
        return Err(
            "Managed mode enabled: explicit extension URL installation is not allowed".to_string(),
        );
    }
    Ok(())
}

async fn ensure_extension_allowed_in_managed_mode(
    state: &AppState,
    name: &str,
) -> Result<(), String> {
    if !managed_mode_enabled() {
        return Ok(());
    }

    if let Some(db) = state.db.as_ref() {
        if let Some(policy) = load_verified_policy_from_store(db.as_ref(), &state.scope_id).await? {
            if policy.allows_extension(name) {
                return Ok(());
            }
            return Err(format!(
                "Managed mode enabled: extension '{}' is not in signed policy allowlist",
                name
            ));
        }
    }

    let allowed = load_string_set(state, MANAGED_ALLOWED_EXTENSIONS_SETTING_KEY).await?;
    if allowed.contains(name) {
        return Ok(());
    }

    Err(format!(
        "Managed mode enabled: extension '{}' is not in approved allowlist",
        name
    ))
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
        .list(None, include_available.unwrap_or(false), &state.scope_id)
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
    ensure_extension_allowed_in_managed_mode(state, &name).await?;
    ext_mgr
        .activate(&name, &state.scope_id)
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
        if let Err(reactivate_error) = ext_mgr.activate(&name, &state.scope_id).await {
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
    validate_extension_install_source(managed_mode_enabled(), url.as_deref())?;
    ensure_extension_allowed_in_managed_mode(state, &name).await?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let result = ext_mgr
        .install(&name, url.as_deref(), None, &state.scope_id)
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
        .remove(&name, &state.scope_id)
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
        .get_setup_schema(&name, &state.scope_id)
        .await
        .map_err(|e| format!("Failed to get setup schema: {}", e))?;

    let kind = ext_mgr
        .list(None, false, &state.scope_id)
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
            &state.scope_id,
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
        .list(None, false, &state.scope_id)
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
        return Err(format!("{}; rollback failed: {}", error, rollback_error));
    }
    Ok(())
}

async fn soft_deactivate_extension_runtime(state: &AppState, name: &str) -> Result<(), String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let installed = ext_mgr
        .list(None, false, &state.scope_id)
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
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::{
        effective_active, extension_name_exists, set_extension_enabled_with_persist,
        validate_extension_install_source,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::AppState;
    use dasclaw_runtime::context::ContextManager;
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;

    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }

        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }

        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    fn create_test_app_state(disabled_extensions: &[&str]) -> AppState {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));
        let safety_bridge = Arc::new(SafetyBridge::new(Arc::clone(&safety), None, None));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );
        let tools = Arc::new(ToolRegistry::new());
        let context_manager = Arc::new(ContextManager::new(5));
        let data_reporter = Arc::new(crate::data_reporter::DataReporter::new(
            "http://localhost:3000".to_string(),
            "test-token".to_string(),
        ));

        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        AppState {
            msg_sender: tx,
            db: None,
            workspace: None,
            tools,
            extension_manager: None,
            skill_registry: None,
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
            context_manager,
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "test-owner".to_string(),
            )),
            data_reporter,
            scope_id: "test-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: Arc::clone(&stub_llm),
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(
                disabled_extensions
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
            ),
        }
    }

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

    #[tokio::test]
    async fn test_disable_extension_rollback_when_persist_fails() {
        let state = create_test_app_state(&[]);

        let result = set_extension_enabled_with_persist(&state, "github", false).await;
        assert!(result.is_err());
        let message = result.err().unwrap_or_default();
        assert!(
            message.contains("Database not available"),
            "expected persistence error, actual: {}",
            message
        );

        assert!(
            state.extension_enabled("github"),
            "failed persist should rollback disable operation"
        );

        let snapshot = state
            .disabled_extensions_snapshot()
            .expect("snapshot should be readable");
        assert!(snapshot.is_empty(), "rollback should clear disabled flag");
    }

    #[tokio::test]
    async fn test_enable_extension_rollback_when_persist_fails() {
        let state = create_test_app_state(&["github"]);

        let result = set_extension_enabled_with_persist(&state, "github", true).await;
        assert!(result.is_err());
        let message = result.err().unwrap_or_default();
        assert!(
            message.contains("Database not available"),
            "expected persistence error, actual: {}",
            message
        );

        assert!(
            !state.extension_enabled("github"),
            "failed persist should rollback enable operation"
        );

        let snapshot = state
            .disabled_extensions_snapshot()
            .expect("snapshot should be readable");
        assert_eq!(snapshot, vec!["github".to_string()]);
    }

    #[test]
    fn test_validate_extension_install_source_managed_mode_rejects_url() {
        let result = validate_extension_install_source(true, Some("https://example.com/ext.wasm"));
        assert!(
            result.is_err(),
            "managed mode should reject explicit extension URL"
        );
        let message = result.err().unwrap_or_default();
        assert!(
            message.contains("Managed mode"),
            "error should mention managed mode, actual: {}",
            message
        );
    }

    #[test]
    fn test_validate_extension_install_source_managed_mode_accepts_registry_install() {
        let result = validate_extension_install_source(true, None);
        assert!(
            result.is_ok(),
            "managed mode should allow registry-based extension install"
        );
    }

    #[test]
    fn test_validate_extension_install_source_non_managed_mode_allows_url() {
        let result = validate_extension_install_source(false, Some("https://example.com/ext.wasm"));
        assert!(
            result.is_ok(),
            "non-managed mode should allow explicit extension URL"
        );
    }
}
