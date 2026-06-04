//! 扩展管理 Tauri Commands。
//!
//! 直接调用 `ExtensionManager` 管理扩展（MCP、WASM 等）。

use serde::{Deserialize, Serialize};
use tauri::State;

use super::persistence::{load_string_set, persist_disabled_items};
use crate::managed_policy::load_verified_policy_from_store;
use crate::state::{AppState, EngineState};
use ironclaw::channels::web::types::{SecretFieldInfo, SetupFieldInfo};
use ironclaw::extensions::{ConfigureResult, InstalledExtension, SearchResult};

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

fn installed_extension_to_info(
    extension: InstalledExtension,
    is_soft_enabled: bool,
) -> ExtensionInfo {
    ExtensionInfo {
        name: extension.name,
        display_name: extension.display_name,
        kind: extension.kind.to_string(),
        installed: extension.installed,
        active: effective_active(extension.active, is_soft_enabled),
        authenticated: extension.authenticated,
        tools: extension.tools,
    }
}

fn setup_field_to_response_field(field: SetupFieldInfo) -> ExtensionSetupField {
    ExtensionSetupField {
        name: field.name,
        prompt: field.prompt,
        optional: field.optional,
        provided: field.provided,
        input_type: format!("{:?}", field.input_type),
        auto_generate: false,
    }
}

fn secret_field_to_response_field(field: SecretFieldInfo) -> ExtensionSetupField {
    ExtensionSetupField {
        name: field.name,
        prompt: field.prompt,
        optional: field.optional,
        provided: field.provided,
        input_type: if field.auto_generate {
            "AutoGenerate".to_string()
        } else {
            "Password".to_string()
        },
        auto_generate: field.auto_generate,
    }
}

fn setup_submit_response_from_result(result: ConfigureResult) -> ExtensionSetupSubmitResponse {
    ExtensionSetupSubmitResponse {
        success: result.activated || result.verification.is_some(),
        message: result.message,
        activated: result.activated,
        auth_url: result.auth_url,
    }
}

fn search_result_to_info(result: SearchResult) -> ExtensionInfo {
    ExtensionInfo {
        name: result.entry.name,
        display_name: Some(result.entry.display_name),
        kind: result.entry.kind.to_string(),
        installed: false,
        active: false,
        authenticated: false,
        tools: Vec::new(),
    }
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
pub(crate) async fn list_extensions_from_state(
    state: &AppState,
    include_available: Option<bool>,
) -> Result<Vec<ExtensionInfo>, String> {
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
        .map(|extension| {
            let is_soft_enabled = state.extension_enabled(&extension.name);
            installed_extension_to_info(extension, is_soft_enabled)
        })
        .collect())
}

#[tauri::command]
pub async fn ic_list_extensions(
    state: State<'_, EngineState>,
    include_available: Option<bool>,
) -> Result<Vec<ExtensionInfo>, String> {
    list_extensions_from_state(state.get()?, include_available).await
}

/// 启用扩展。
#[tauri::command]
pub async fn ic_enable_extension(
    state: State<'_, EngineState>,
    name: String,
) -> Result<(), String> {
    enable_extension_from_state(state.get()?, &name).await
}

pub(crate) async fn enable_extension_from_state(
    state: &AppState,
    name: &str,
) -> Result<(), String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    ensure_extension_installed(state, name).await?;
    ensure_extension_allowed_in_managed_mode(state, name).await?;
    ext_mgr
        .activate(name, &state.scope_id)
        .await
        .map_err(|e| format!("Failed to enable extension: {}", e))?;
    if let Err(error) = set_extension_enabled_with_persist(state, name, true).await {
        let _ = soft_deactivate_extension_runtime(state, name).await;
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
    disable_extension_from_state(state.get()?, &name).await
}

pub(crate) async fn disable_extension_from_state(
    state: &AppState,
    name: &str,
) -> Result<(), String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    ensure_extension_installed(state, name).await?;
    soft_deactivate_extension_runtime(state, name).await?;
    if let Err(error) = set_extension_enabled_with_persist(state, name, false).await {
        if let Err(reactivate_error) = ext_mgr.activate(name, &state.scope_id).await {
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
    install_extension_from_state(state.get()?, &name, url.as_deref()).await
}

pub(crate) async fn install_extension_from_state(
    state: &AppState,
    name: &str,
    url: Option<&str>,
) -> Result<String, String> {
    validate_extension_install_source(managed_mode_enabled(), url)?;
    ensure_extension_allowed_in_managed_mode(state, name).await?;
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let result = ext_mgr
        .install(name, url, None, &state.scope_id)
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
    uninstall_extension_from_state(state.get()?, &name).await
}

pub(crate) async fn uninstall_extension_from_state(
    state: &AppState,
    name: &str,
) -> Result<String, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let message = ext_mgr
        .remove(name, &state.scope_id)
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
    pub auto_generate: bool,
}

/// 扩展配置 Schema 响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSetupResponse {
    pub name: String,
    pub kind: String,
    pub secrets: Vec<ExtensionSetupField>,
    pub fields: Vec<ExtensionSetupField>,
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
    extension_setup_from_state(state.get()?, &name).await
}

pub(crate) async fn extension_setup_from_state(
    state: &AppState,
    name: &str,
) -> Result<ExtensionSetupResponse, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let setup = ext_mgr
        .get_setup_schema(name, &state.scope_id)
        .await
        .map_err(|e| format!("Failed to get setup schema: {}", e))?;

    let kind = ext_mgr
        .list(None, false, &state.scope_id)
        .await
        .ok()
        .and_then(|list| list.into_iter().find(|e| e.name == name))
        .map(|e| e.kind.to_string())
        .unwrap_or_default();

    let secrets: Vec<ExtensionSetupField> = setup
        .secrets
        .into_iter()
        .map(secret_field_to_response_field)
        .collect();
    let fields: Vec<ExtensionSetupField> = setup
        .fields
        .into_iter()
        .map(setup_field_to_response_field)
        .collect();

    tracing::debug!(extension = %name, fields = fields.len(), "Extension setup schema loaded");

    Ok(ExtensionSetupResponse {
        name: name.to_string(),
        kind,
        secrets,
        fields,
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
    fields: Option<std::collections::HashMap<String, String>>,
) -> Result<ExtensionSetupSubmitResponse, String> {
    let fields = fields.unwrap_or_default();
    extension_setup_submit_from_state(state.get()?, &name, &secrets, &fields).await
}

pub(crate) async fn extension_setup_submit_from_state(
    state: &AppState,
    name: &str,
    secrets: &std::collections::HashMap<String, String>,
    fields: &std::collections::HashMap<String, String>,
) -> Result<ExtensionSetupSubmitResponse, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let result = ext_mgr
        .configure(name, secrets, fields, &state.scope_id)
        .await
        .map_err(|e| format!("Failed to configure extension: {}", e))?;

    tracing::info!(
        extension = %name,
        activated = result.activated,
        "Extension configured"
    );

    Ok(setup_submit_response_from_result(result))
}

/// 搜索扩展。
#[tauri::command]
pub async fn ic_search_extensions(
    state: State<'_, EngineState>,
    query: String,
) -> Result<Vec<ExtensionInfo>, String> {
    search_extensions_from_state(state.get()?, &query).await
}

pub(crate) async fn search_extensions_from_state(
    state: &AppState,
    query: &str,
) -> Result<Vec<ExtensionInfo>, String> {
    let ext_mgr = state
        .extension_manager
        .as_ref()
        .ok_or("Extension manager not available")?;

    let results = ext_mgr
        .search(query, true)
        .await
        .map_err(|e| format!("Failed to search extensions: {}", e))?;

    Ok(results.into_iter().map(search_result_to_info).collect())
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
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use super::{
        disable_extension_from_state, effective_active, enable_extension_from_state,
        extension_name_exists, extension_setup_from_state, extension_setup_submit_from_state,
        install_extension_from_state, installed_extension_to_info, list_extensions_from_state,
        search_extensions_from_state, search_result_to_info, secret_field_to_response_field,
        set_extension_enabled_with_persist, setup_field_to_response_field,
        setup_submit_response_from_result, uninstall_extension_from_state,
        validate_extension_install_source, DISABLED_EXTENSIONS_SETTING_KEY,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::AppState;
    use dasclaw_runtime::context::ContextManager;
    use dasclaw_runtime::secrets::{InMemorySecretsStore, SecretsStore};
    use ironclaw::extensions::{AuthHint, ExtensionKind, ExtensionSource, RegistryEntry};
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
        create_test_app_state_with_components(
            disabled_extensions,
            None,
            None,
            Arc::new(ToolRegistry::new()),
        )
    }

    fn create_test_app_state_with_components(
        disabled_extensions: &[&str],
        db: Option<Arc<dyn ironclaw::db::Database>>,
        extension_manager: Option<Arc<ironclaw::extensions::ExtensionManager>>,
        tools: Arc<ToolRegistry>,
    ) -> AppState {
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
        let context_manager = Arc::new(ContextManager::new(5));
        let data_reporter = Arc::new(crate::data_reporter::DataReporter::new_for_test());

        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        AppState {
            msg_sender: tx,
            db,
            workspace: None,
            tools,
            extension_manager,
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

    async fn test_db(tempdir: &tempfile::TempDir) -> Arc<dyn ironclaw::db::Database> {
        let db_path = tempdir.path().join("extensions-state.db");
        let backend = ironclaw::db::libsql::LibSqlBackend::new_local(&db_path)
            .await
            .expect("file-backed libsql backend should initialize");
        <ironclaw::db::libsql::LibSqlBackend as ironclaw::db::Database>::run_migrations(&backend)
            .await
            .expect("migrations should run");
        Arc::new(backend)
    }

    fn channel_relay_entry(name: &str) -> RegistryEntry {
        RegistryEntry {
            name: name.to_string(),
            display_name: "Roundtrip Relay".to_string(),
            kind: ExtensionKind::ChannelRelay,
            description: "Roundtrip channel relay extension".to_string(),
            keywords: vec!["roundtrip".to_string(), "relay".to_string()],
            source: ExtensionSource::ChannelRelay {
                relay_url: "https://relay.example.test".to_string(),
            },
            fallback_source: None,
            auth_hint: AuthHint::ChannelRelayOAuth,
            version: Some("1.0.0".to_string()),
        }
    }

    fn test_secrets_store() -> Arc<dyn SecretsStore + Send + Sync> {
        let master_key = ironclaw::secrets::keychain::generate_master_key_hex();
        let crypto =
            ironclaw::secrets::crypto_from_hex(&master_key).expect("test crypto should initialize");
        Arc::new(InMemorySecretsStore::new(crypto))
    }

    fn test_extension_manager(
        tempdir: &tempfile::TempDir,
        db: Arc<dyn ironclaw::db::Database>,
        tools: Arc<ToolRegistry>,
        catalog_entries: Vec<RegistryEntry>,
    ) -> Arc<ironclaw::extensions::ExtensionManager> {
        Arc::new(ironclaw::extensions::ExtensionManager::new(
            Arc::new(ironclaw::tools::mcp::session::McpSessionManager::new()),
            Arc::new(ironclaw::tools::mcp::McpProcessManager::new()),
            test_secrets_store(),
            tools,
            None,
            None,
            tempdir.path().join("wasm-tools"),
            tempdir.path().join("wasm-channels"),
            None,
            "test-owner".to_string(),
            Some(db),
            catalog_entries,
        ))
    }

    async fn test_app_state_with_extensions(
        catalog_entries: Vec<RegistryEntry>,
    ) -> (AppState, tempfile::TempDir) {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let db = test_db(&tempdir).await;
        let tools = Arc::new(ToolRegistry::new());
        let extension_manager = test_extension_manager(
            &tempdir,
            Arc::clone(&db),
            Arc::clone(&tools),
            catalog_entries,
        );
        let state =
            create_test_app_state_with_components(&[], Some(db), Some(extension_manager), tools);
        (state, tempdir)
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

    #[test]
    fn req_extensions_list_mapping_combines_runtime_and_soft_enable() {
        let extension = ironclaw::extensions::InstalledExtension {
            name: "github".to_string(),
            kind: ironclaw::extensions::ExtensionKind::McpServer,
            display_name: Some("GitHub".to_string()),
            description: Some("GitHub MCP".to_string()),
            url: Some("https://mcp.example.com".to_string()),
            authenticated: true,
            active: true,
            tools: vec!["repo_search".to_string()],
            needs_setup: true,
            has_auth: true,
            installed: true,
            activation_error: Some("transient".to_string()),
            version: Some("1.2.3".to_string()),
        };

        let info = installed_extension_to_info(extension, false);

        assert_eq!(info.name, "github");
        assert_eq!(info.display_name.as_deref(), Some("GitHub"));
        assert_eq!(info.kind, "mcp_server");
        assert!(info.installed);
        assert!(
            !info.active,
            "soft-disabled extensions must render inactive"
        );
        assert!(info.authenticated);
        assert_eq!(info.tools, vec!["repo_search".to_string()]);
    }

    #[test]
    fn req_extensions_search_mapping_marks_results_available_not_installed() {
        let result = ironclaw::extensions::SearchResult {
            entry: ironclaw::extensions::RegistryEntry {
                name: "slack".to_string(),
                display_name: "Slack".to_string(),
                kind: ironclaw::extensions::ExtensionKind::ChannelRelay,
                description: "Team messages".to_string(),
                keywords: vec!["chat".to_string()],
                source: ironclaw::extensions::ExtensionSource::ChannelRelay {
                    relay_url: "https://relay.example.com".to_string(),
                },
                fallback_source: None,
                auth_hint: ironclaw::extensions::AuthHint::ChannelRelayOAuth,
                version: Some("0.3.0".to_string()),
            },
            source: ironclaw::extensions::ResultSource::Registry,
            validated: true,
        };

        let info = search_result_to_info(result);

        assert_eq!(info.name, "slack");
        assert_eq!(info.display_name.as_deref(), Some("Slack"));
        assert_eq!(info.kind, "channel_relay");
        assert!(!info.installed);
        assert!(!info.active);
        assert!(!info.authenticated);
        assert!(info.tools.is_empty());
    }

    #[test]
    fn req_extensions_setup_field_mapping_preserves_prompt_state() {
        let field = ironclaw::channels::web::types::SetupFieldInfo {
            name: "api_key".to_string(),
            prompt: "API key".to_string(),
            optional: false,
            provided: true,
            input_type: ironclaw::tools::wasm::ToolSetupFieldInputType::Password,
        };

        let response_field = setup_field_to_response_field(field);

        assert_eq!(response_field.name, "api_key");
        assert_eq!(response_field.prompt, "API key");
        assert!(!response_field.optional);
        assert!(response_field.provided);
        assert_eq!(response_field.input_type, "Password");
        assert!(!response_field.auto_generate);
    }

    #[test]
    fn req_extensions_setup_secret_mapping_preserves_auto_generate_state() {
        let field = ironclaw::channels::web::types::SecretFieldInfo {
            name: "webhook_secret".to_string(),
            prompt: "Webhook secret".to_string(),
            optional: true,
            provided: false,
            auto_generate: true,
        };

        let response_field = secret_field_to_response_field(field);

        assert_eq!(response_field.name, "webhook_secret");
        assert_eq!(response_field.input_type, "AutoGenerate");
        assert!(response_field.auto_generate);
    }

    #[test]
    fn req_extensions_setup_submit_succeeds_for_manual_verification() {
        let result = ironclaw::extensions::ConfigureResult {
            message: "Verification required".to_string(),
            activated: false,
            restart_required: false,
            auth_url: None,
            verification: Some(ironclaw::extensions::VerificationChallenge {
                code: "ABC123".to_string(),
                instructions: "Send this code".to_string(),
                deep_link: None,
            }),
        };

        let response = setup_submit_response_from_result(result);

        assert!(response.success);
        assert_eq!(response.message, "Verification required");
        assert!(!response.activated);
        assert!(response.auth_url.is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_extensions_channel_relay_state_round_trip() {
        std::env::remove_var("MANAGED_MODE");

        let extension_name = "roundtrip-relay";
        let (state, _tempdir) =
            test_app_state_with_extensions(vec![channel_relay_entry(extension_name)]).await;

        let search_results = search_extensions_from_state(&state, "roundtrip")
            .await
            .expect("search should read registry entries from real manager");
        let result = search_results
            .iter()
            .find(|extension| extension.name == extension_name)
            .expect("registry extension should be searchable before install");
        assert!(!result.installed);
        assert_eq!(result.kind, "channel_relay");

        let install_message = install_extension_from_state(&state, extension_name, None)
            .await
            .expect("registry channel relay install should update manager state");
        assert!(
            install_message.contains("installed"),
            "install response should come from real manager, actual: {install_message}"
        );

        let listed = list_extensions_from_state(&state, Some(false))
            .await
            .expect("list should read installed manager state");
        let extension = listed
            .iter()
            .find(|extension| extension.name == extension_name)
            .expect("installed extension should be listed");
        assert!(extension.installed);
        assert_eq!(extension.display_name.as_deref(), Some("Roundtrip Relay"));
        assert_eq!(extension.kind, "channel_relay");
        assert!(!extension.active, "relay is not active before auth/setup");

        let setup = extension_setup_from_state(&state, extension_name)
            .await
            .expect("setup schema should be loaded from installed extension");
        assert_eq!(setup.name, extension_name);
        assert_eq!(setup.kind, "channel_relay");
        let relay_url_field = setup
            .fields
            .iter()
            .find(|field| field.name == "relay_url")
            .expect("channel relay setup should expose relay_url field");
        assert_eq!(relay_url_field.input_type, "Text");
        assert!(!relay_url_field.provided);

        let relay_url = "https://relay.local.test";
        let setup_response = extension_setup_submit_from_state(
            &state,
            extension_name,
            &HashMap::new(),
            &HashMap::from([("relay_url".to_string(), relay_url.to_string())]),
        )
        .await
        .expect("setup submit should save relay_url even when activation cannot complete");
        assert!(!setup_response.success);
        assert!(!setup_response.activated);
        assert!(
            setup_response.message.contains("Configuration saved"),
            "setup response should distinguish saved config from activation failure: {}",
            setup_response.message
        );

        let relay_setting = state
            .db
            .as_ref()
            .expect("db should exist")
            .get_setting(
                &state.scope_id,
                &format!("extensions.{extension_name}.relay_url"),
            )
            .await
            .expect("relay_url setting should be readable")
            .expect("relay_url setting should be persisted");
        assert_eq!(relay_setting, serde_json::json!(relay_url));

        let setup = extension_setup_from_state(&state, extension_name)
            .await
            .expect("setup schema should reflect persisted relay_url");
        let relay_url_field = setup
            .fields
            .iter()
            .find(|field| field.name == "relay_url")
            .expect("channel relay setup should still expose relay_url field");
        assert!(relay_url_field.provided);

        disable_extension_from_state(&state, extension_name)
            .await
            .expect("disable should persist soft-disabled state");
        let disabled = state
            .db
            .as_ref()
            .expect("db should exist")
            .get_setting(&state.scope_id, DISABLED_EXTENSIONS_SETTING_KEY)
            .await
            .expect("disabled extension setting should be readable")
            .expect("disabled extension setting should be persisted");
        assert_eq!(disabled, serde_json::json!([extension_name]));

        let enable_error = enable_extension_from_state(&state, extension_name)
            .await
            .expect_err("enable should fail without relay auth/team configuration");
        assert!(
            enable_error.contains("Failed to enable extension"),
            "enable failure should come from real activation path, actual: {enable_error}"
        );
        let disabled = state
            .db
            .as_ref()
            .expect("db should exist")
            .get_setting(&state.scope_id, DISABLED_EXTENSIONS_SETTING_KEY)
            .await
            .expect("disabled extension setting should remain readable")
            .expect("disabled extension setting should remain persisted");
        assert_eq!(
            disabled,
            serde_json::json!([extension_name]),
            "failed enable must not clear the persisted soft-disabled state"
        );

        let uninstall_message = uninstall_extension_from_state(&state, extension_name)
            .await
            .expect("uninstall should remove extension from manager state");
        assert!(
            uninstall_message.contains("Removed"),
            "uninstall response should come from real manager, actual: {uninstall_message}"
        );
        let listed = list_extensions_from_state(&state, Some(false))
            .await
            .expect("list should read post-uninstall manager state");
        assert!(
            listed
                .iter()
                .all(|extension| extension.name != extension_name),
            "uninstalled extension should no longer be listed"
        );
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
