use crate::auth::AuthManager;
use crate::storage::StorageManager;
use crate::extension_manager::ExtensionManager;
use crate::routine_manager::RoutineManager;
use crate::memory_manager::{is_protected_file, DeleteResult, MemoryApiExtensions};
use crate::dlp::{DlpIntegration, DlpIntegrationConfig, DlpStatistics, SanitizationResult, CustomPatternConfig};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use tauri::Emitter; // 添加 Emitter trait

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockResponse {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub user_id: String,
    pub created_at: u64,
    pub last_activity: u64,
    pub is_expired: bool,
}

#[derive(Clone)]
pub struct CommandState {
    pub auth_manager: Arc<Mutex<AuthManager>>,
    pub storage_manager: Arc<Mutex<Option<StorageManager>>>,
    pub extension_manager: Arc<StdMutex<ExtensionManager>>,
    pub routine_manager: Arc<StdMutex<RoutineManager>>,
    pub skill_manager: Arc<StdMutex<crate::skill_manager::SkillManager>>,
    pub api_client: Arc<crate::api_client::ApiClient>,
    pub dlp_integration: Arc<Mutex<DlpIntegration>>,
}

impl CommandState {
    pub fn new() -> Self {
        // Default to embedded server port for development
        let api_client = crate::api_client::ApiClient::new(
            format!("http://localhost:{}", crate::embedded_server::EMBEDDED_SERVER_PORT)
        );
        
        // 初始化 DLP 集成（使用 blocking 方式）
        let dlp_integration = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                DlpIntegration::with_default_config().await
                    .expect("Failed to initialize DLP integration")
            })
        });
        
        Self {
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            storage_manager: Arc::new(Mutex::new(None)),
            extension_manager: Arc::new(StdMutex::new(ExtensionManager::new())),
            routine_manager: Arc::new(StdMutex::new(RoutineManager::new())),
            skill_manager: Arc::new(StdMutex::new(crate::skill_manager::SkillManager::new())),
            api_client: Arc::new(api_client),
            dlp_integration: Arc::new(Mutex::new(dlp_integration)),
        }
    }
    
    pub fn new_with_token(auth_token: String) -> Self {
        // 使用提供的 token 创建 ApiClient
        let api_client = crate::api_client::ApiClient::new_with_token(
            format!("http://localhost:{}", crate::embedded_server::EMBEDDED_SERVER_PORT),
            auth_token
        );
        
        // 初始化 DLP 集成（使用 blocking 方式）
        let dlp_integration = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                DlpIntegration::with_default_config().await
                    .expect("Failed to initialize DLP integration")
            })
        });
        
        Self {
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            storage_manager: Arc::new(Mutex::new(None)),
            extension_manager: Arc::new(StdMutex::new(ExtensionManager::new())),
            routine_manager: Arc::new(StdMutex::new(RoutineManager::new())),
            skill_manager: Arc::new(StdMutex::new(crate::skill_manager::SkillManager::new())),
            api_client: Arc::new(api_client),
            dlp_integration: Arc::new(Mutex::new(dlp_integration)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub password_set: bool,
}

#[tauri::command]
pub async fn check_setup_status(
    state: tauri::State<'_, CommandState>,
) -> Result<SetupStatus> {
    let auth = state.auth_manager.lock().await;
    // Check if master password is set by trying to verify an empty password
    // If it fails with "Master password not set", then password is not set
    let password_set = auth.verify_password("").is_ok() || 
                       !matches!(auth.verify_password(""), Err(Error::AuthError(_)));
    
    Ok(SetupStatus {
        password_set: password_set || false, // Conservative: assume not set if we can't verify
    })
}

#[tauri::command]
pub async fn setup_master_password(
    password: String,
    state: tauri::State<'_, CommandState>,
) -> Result<SetupResponse> {
    let mut auth = state.auth_manager.lock().await;

    match auth.setup_master_password(&password) {
        Ok(_) => Ok(SetupResponse {
            success: true,
            message: "Master password set successfully".to_string(),
        }),
        Err(e) => Ok(SetupResponse {
            success: false,
            message: format!("Failed to set master password: {}", e),
        }),
    }
}

#[tauri::command]
pub async fn unlock_app(
    password: String,
    state: tauri::State<'_, CommandState>,
) -> Result<UnlockResponse> {
    let mut auth = state.auth_manager.lock().await;

    match auth.verify_password(&password) {
        Ok(_) => {
            let session = auth.create_session("user".to_string())?;
            Ok(UnlockResponse {
                success: true,
                session_id: Some(session.user_id),
                message: "Unlocked successfully".to_string(),
            })
        }
        Err(e) => Ok(UnlockResponse {
            success: false,
            session_id: None,
            message: format!("Unlock failed: {}", e),
        }),
    }
}

#[tauri::command]
pub async fn get_session_info(
    state: tauri::State<'_, CommandState>,
) -> Result<Option<SessionInfo>> {
    let auth = state.auth_manager.lock().await;

    match auth.get_session() {
        Ok(session) => Ok(Some(SessionInfo {
            user_id: session.user_id.clone(),
            created_at: session.created_at,
            last_activity: session.last_activity,
            is_expired: session.is_expired(),
        })),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn lock_app(state: tauri::State<'_, CommandState>) -> Result<()> {
    let mut auth = state.auth_manager.lock().await;
    auth.logout();
    Ok(())
}

#[tauri::command]
pub async fn update_session_activity(
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let mut auth = state.auth_manager.lock().await;
    auth.update_session_activity()?;
    Ok(())
}

#[tauri::command]
pub async fn store_config(
    key: String,
    value: String,
    encrypt: bool,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.set_config(&key, &value, encrypt).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn get_config(
    key: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Option<String>> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.get_config(&key).await
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn log_audit_event(
    action: String,
    details: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.add_audit_log(&action, &details).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn get_audit_logs(
    limit: i64,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::storage::AuditLog>> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.get_audit_logs(limit).await
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}


// --- Plugin Management Commands ---

#[tauri::command]
pub async fn get_installed_plugins(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<serde_json::Value>> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let plugins = manager.get_installed_plugins().await?;
        Ok(plugins)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn get_available_plugins(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<serde_json::Value>> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let plugins = manager.get_available_plugins().await?;
        Ok(plugins)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn check_plugin_updates(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<serde_json::Value>> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let updates = manager.check_plugin_updates().await?;
        Ok(updates)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn install_plugin(
    plugin_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.install_plugin(&plugin_id).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn uninstall_plugin(
    plugin_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.uninstall_plugin(&plugin_id).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn enable_plugin(
    plugin_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.enable_plugin(&plugin_id).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn disable_plugin(
    plugin_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.disable_plugin(&plugin_id).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn update_plugin(
    plugin_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.update_plugin(&plugin_id).await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

// --- Offline Mode Commands ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineState {
    pub is_offline: bool,
    pub last_sync: u64,
    pub cached_data_size_mb: u32,
    pub network_status: String,
}

#[tauri::command]
pub async fn get_offline_state(
    state: tauri::State<'_, CommandState>,
) -> Result<OfflineState> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let offline_state = manager.get_offline_state().await?;
        Ok(offline_state)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn enable_offline_mode(
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.enable_offline_mode().await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn disable_offline_mode(
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        manager.disable_offline_mode().await?;
        Ok(())
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn get_offline_capabilities(
    state: tauri::State<'_, CommandState>,
) -> Result<serde_json::Value> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let caps = manager.get_offline_capabilities().await?;
        Ok(caps)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

#[tauri::command]
pub async fn can_perform_operation(
    operation: String,
    state: tauri::State<'_, CommandState>,
) -> Result<bool> {
    let storage = state.storage_manager.lock().await;

    if let Some(manager) = storage.as_ref() {
        let allowed = manager.can_perform_operation(&operation).await?;
        Ok(allowed)
    } else {
        Err(Error::StorageError("Storage not initialized".to_string()))
    }
}

// --- Chat and Thread Commands ---
#[tauri::command]
pub async fn get_threads(
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::ThreadListResponse> {
    // 诊断日志
    tracing::info!("🔍 get_threads called");
    
    // 检查 ApiClient 的 token
    let token = state.api_client.auth_token();
    let token_len = token.len();
    tracing::info!("   ApiClient token length: {}", token_len);
    
    if token_len == 0 {
        tracing::error!("❌ ApiClient has empty token!");
        return Err(crate::Error::ConfigError("Empty auth token".to_string()));
    }
    
    if token_len != 64 {
        tracing::error!("❌ ApiClient token length is {} (expected 64)", token_len);
        tracing::error!("   Token (first 16): {}", &token[..token_len.min(16)]);
        tracing::error!("   Token (last 16): {}", &token[token_len.saturating_sub(16)..]);
    } else {
        tracing::info!("✅ ApiClient token length is correct (64)");
        tracing::debug!("   Token (first 16): {}", &token[..16]);
        tracing::debug!("   Token (last 16): {}", &token[48..]);
    }
    
    // 测试 HTTP header 构建
    let auth_header = format!("Bearer {}", token);
    tracing::info!("   Auth header: {:?}", auth_header);
    
    // 检查 header 中的字符
    for (i, ch) in auth_header.chars().enumerate() {
        if ch.is_control() {
            tracing::error!("   控制字符在位置 {}: {:?} (代码: {})", i, ch, ch as u32);
        }
        if !ch.is_ascii() {
            tracing::error!("   非ASCII字符在位置 {}: {:?} (代码: {})", i, ch, ch as u32);
        }
    }
    
    // 测试 HeaderValue 创建
    match reqwest::header::HeaderValue::from_str(&auth_header) {
        Ok(header_value) => {
            tracing::info!("✅ HeaderValue 创建成功: {:?}", header_value);
        }
        Err(e) => {
            tracing::error!("❌ HeaderValue 创建失败: {}", e);
            return Err(crate::Error::SerializationError(format!("Invalid header value: {}", e)));
        }
    }
    
    state.api_client.get_threads().await
}

#[tauri::command]
pub async fn create_thread(
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::ThreadInfo> {
    state.api_client.create_thread().await
}

#[tauri::command]
pub async fn send_message(
    thread_id: String,
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::SendMessageResponse> {
    let req = crate::api_client::SendMessageRequest {
        content,
        thread_id: Some(thread_id),
    };
    state.api_client.send_message(req).await
}

#[tauri::command]
pub async fn get_messages(
    thread_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::Message>> {
    state.api_client.get_messages(&thread_id).await
}

#[tauri::command]
pub async fn approve_operation(
    request_id: String,
    action: String,
    thread_id: Option<String>,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::SendMessageResponse> {
    let req = crate::api_client::ApprovalRequest {
        request_id,
        action,
        thread_id,
    };
    state.api_client.approve_operation(req).await
}

#[tauri::command]
pub async fn deny_operation(
    request_id: String,
    thread_id: Option<String>,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::SendMessageResponse> {
    let req = crate::api_client::ApprovalRequest {
        request_id,
        action: "deny".to_string(),
        thread_id,
    };
    state.api_client.approve_operation(req).await
}

// Extension Management Commands
#[tauri::command]
pub async fn get_installed_extensions(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::extension_manager::InstalledExtension>> {
    Ok(state.extension_manager.lock().unwrap().get_installed_extensions())
}

#[tauri::command]
pub async fn get_available_extensions(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::extension_manager::ExtensionMetadata>> {
    Ok(state.extension_manager.lock().unwrap().get_available_extensions().to_vec())
}

#[tauri::command]
pub async fn install_extension(
    metadata: crate::extension_manager::ExtensionMetadata,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.extension_manager.lock().unwrap().install_extension(metadata)
}

#[tauri::command]
pub async fn uninstall_extension(
    extension_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.extension_manager.lock().unwrap().uninstall_extension(&extension_id)
}

#[tauri::command]
pub async fn enable_extension(
    extension_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.extension_manager.lock().unwrap().enable_extension(&extension_id)
}

#[tauri::command]
pub async fn disable_extension(
    extension_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.extension_manager.lock().unwrap().disable_extension(&extension_id)
}

#[tauri::command]
pub async fn search_extensions(
    query: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::extension_manager::ExtensionMetadata>> {
    let manager = state.extension_manager.lock().unwrap();
    Ok(manager.search_extensions(&query).into_iter().cloned().collect())
}

#[tauri::command]
pub async fn get_enabled_tools(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<String>> {
    Ok(state.extension_manager.lock().unwrap().get_enabled_tools())
}

// Skill Management Commands
#[tauri::command]
pub async fn get_available_skills(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::skill_manager::Skill>> {
    let manager = state.skill_manager.lock().unwrap();
    manager.get_available_skills()
}

#[tauri::command]
pub async fn get_installed_skills(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::skill_manager::InstalledSkill>> {
    let manager = state.skill_manager.lock().unwrap();
    manager.get_installed_skills()
}

#[tauri::command]
pub async fn install_skill(
    skill_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let mut manager = state.skill_manager.lock().unwrap();
    manager.install_skill(skill_id)
}

#[tauri::command]
pub async fn uninstall_skill(
    skill_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let mut manager = state.skill_manager.lock().unwrap();
    manager.uninstall_skill(skill_id)
}

#[tauri::command]
pub async fn enable_skill(
    skill_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let mut manager = state.skill_manager.lock().unwrap();
    manager.enable_skill(skill_id)
}

#[tauri::command]
pub async fn disable_skill(
    skill_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let mut manager = state.skill_manager.lock().unwrap();
    manager.disable_skill(skill_id)
}

// Routine Management Commands
#[tauri::command]
pub async fn get_routines(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::routine_manager::Routine>> {
    let manager = state.routine_manager.lock().unwrap();
    Ok(manager.get_all_routines().into_iter().cloned().collect())
}

#[tauri::command]
pub async fn create_routine(
    name: String,
    description: String,
    trigger: crate::routine_manager::RoutineTrigger,
    actions: Vec<crate::routine_manager::RoutineAction>,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::routine_manager::Routine> {
    state.routine_manager.lock().unwrap().create_routine(name, description, trigger, actions)
}

#[tauri::command]
pub async fn delete_routine(
    routine_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.routine_manager.lock().unwrap().delete_routine(&routine_id)
}

#[tauri::command]
pub async fn trigger_routine(
    routine_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::routine_manager::RoutineRun> {
    state.routine_manager.lock().unwrap().trigger_routine(&routine_id)
}

#[tauri::command]
pub async fn enable_routine(
    routine_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.routine_manager.lock().unwrap().enable_routine(&routine_id)
}

#[tauri::command]
pub async fn disable_routine(
    routine_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.routine_manager.lock().unwrap().disable_routine(&routine_id)
}

#[tauri::command]
pub async fn pause_routine(
    routine_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.routine_manager.lock().unwrap().pause_routine(&routine_id)
}

#[tauri::command]
pub async fn get_routine_runs(
    routine_id: String,
    limit: usize,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::routine_manager::RoutineRun>> {
    let manager = state.routine_manager.lock().unwrap();
    Ok(manager.get_routine_runs(&routine_id, limit).into_iter().cloned().collect())
}

// --- Memory Management Commands ---

#[tauri::command]
pub async fn get_memory_tree(
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::MemoryTreeResponse> {
    state.api_client.get_memory_tree().await
}

#[tauri::command]
pub async fn read_memory(
    memory_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::MemoryContent> {
    state.api_client.read_memory(&memory_id).await
}

#[tauri::command]
pub async fn write_memory(
    memory_id: String,
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::MemoryWriteResponse> {
    state.api_client.write_memory(&memory_id, &content).await
}

#[tauri::command]
pub async fn search_memory(
    query: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::SearchHit>> {
    state.api_client.search_memory(&query).await
}

#[tauri::command]
pub async fn delete_memory_local(
    path: String,
    force: bool,
    state: tauri::State<'_, CommandState>,
) -> Result<DeleteResult> {
    state.api_client.delete_memory_safe(&path, force).await
}

#[tauri::command]
pub async fn is_memory_file_protected(
    path: String,
) -> Result<bool> {
    Ok(is_protected_file(&path))
}

// --- Job Management Commands ---

#[tauri::command]
pub async fn get_jobs(
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::JobInfo>> {
    state.api_client.get_jobs().await
}

#[tauri::command]
pub async fn get_job_detail(
    job_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::JobDetail> {
    state.api_client.get_job_detail(&job_id).await
}

#[tauri::command]
pub async fn cancel_job(
    job_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.api_client.cancel_job(&job_id).await
}

#[tauri::command]
pub async fn restart_job(
    job_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.api_client.restart_job(&job_id).await
}

// --- Log Management Commands ---

#[tauri::command]
pub async fn get_logs(
    limit: usize,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::LogEntry>> {
    state.api_client.get_logs(limit).await
}

#[tauri::command]
pub async fn search_logs(
    query: String,
    limit: usize,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::LogEntry>> {
    state.api_client.search_logs(&query, limit).await
}

#[tauri::command]
pub async fn filter_logs(
    level: String,
    module: String,
    limit: usize,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::LogEntry>> {
    state.api_client.filter_logs(&level, &module, limit).await
}

#[tauri::command]
pub async fn export_logs(
    format: String,
    state: tauri::State<'_, CommandState>,
) -> Result<String> {
    state.api_client.export_logs(&format).await
}

// Message editing/deletion commands
#[tauri::command]
pub async fn edit_message(
    thread_id: String,
    message_id: String,
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::Message> {
    state.api_client.edit_message(&thread_id, &message_id, &content).await
}

#[tauri::command]
pub async fn delete_message(
    thread_id: String,
    message_id: String,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.api_client.delete_message(&thread_id, &message_id).await
}

// Log clearing command
#[tauri::command]
pub async fn clear_logs(
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    state.api_client.clear_logs().await
}

// Message search command
#[tauri::command]
pub async fn search_messages(
    thread_id: String,
    query: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::Message>> {
    state.api_client.search_messages(&thread_id, &query).await
}

// Thread export command
#[tauri::command]
pub async fn export_thread(
    thread_id: String,
    format: String,
    state: tauri::State<'_, CommandState>,
) -> Result<String> {
    state.api_client.export_thread(&thread_id, &format).await
}

// File upload command
#[tauri::command]
pub async fn upload_file(
    thread_id: String,
    file_path: String,
    state: tauri::State<'_, CommandState>,
) -> Result<String> {
    state.api_client.upload_file(&thread_id, &file_path).await
}

// ============================================
// 环境和配置管理命令
// ============================================

/// 应用初始化信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInitInfo {
    /// 认证令牌（前8个字符）
    pub auth_token: String,
    /// API 基础 URL
    pub api_base_url: String,
    /// 数据库类型
    pub database_type: String,
    /// 操作系统
    pub os: String,
    /// 日志级别
    pub log_level: String,
    /// 环境类型
    pub environment: String,
}

/// 获取应用初始化信息
#[tauri::command]
pub async fn get_app_init_info() -> Result<AppInitInfo> {
    use crate::{AuthTokenManager, AppConfig, platform_utils};
    
    let token_manager = AuthTokenManager::new();
    let token = token_manager.load_or_generate()
        .map_err(|e| crate::Error::ConfigError(e.to_string()))?;
    
    let config = AppConfig::load_or_default();
    
    Ok(AppInitInfo {
        auth_token: token[..8].to_string(),
        api_base_url: config.api_base_url,
        database_type: config.database_type,
        os: platform_utils::get_os_name().to_string(),
        log_level: config.log_level,
        environment: "development".to_string(), // TODO: 从环境变量读取
    })
}

/// 获取认证令牌
#[tauri::command]
pub async fn get_auth_token() -> Result<String> {
    use crate::AuthTokenManager;
    
    let token_manager = AuthTokenManager::new();
    let token = token_manager.load_or_generate()
        .map_err(|e| crate::Error::ConfigError(e.to_string()))?;
    
    // 验证 token 格式
    if token.len() != 64 {
        tracing::error!("❌ Token length is {} (expected 64)", token.len());
        return Err(crate::Error::ConfigError(format!("Invalid token length: {}", token.len())));
    }
    
    if !token.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::error!("❌ Token contains non-hex characters");
        for (i, ch) in token.chars().enumerate() {
            if !ch.is_ascii_hexdigit() {
                tracing::error!("   Position {}: {:?} (code: {})", i, ch, ch as u32);
            }
        }
        return Err(crate::Error::ConfigError("Token contains non-hex characters".to_string()));
    }
    
    tracing::info!("✅ Token validation passed");
    Ok(token)
}

/// 刷新认证令牌
#[tauri::command]
pub async fn refresh_auth_token() -> Result<String> {
    use crate::AuthTokenManager;
    
    let token_manager = AuthTokenManager::new();
    let new_token = AuthTokenManager::generate_new_token();
    token_manager.save(&new_token)
        .map_err(|e| crate::Error::TokenError(e.to_string()))?;
    
    Ok(new_token)
}

/// 获取应用配置
#[tauri::command]
pub async fn get_app_config() -> Result<crate::AppConfig> {
    Ok(crate::AppConfig::load_or_default())
}

/// 获取网络配置
#[tauri::command]
pub async fn get_network_config() -> Result<NetworkConfigInfo> {
    use crate::NetworkConfig;
    
    let config = NetworkConfig::from_env();
    
    Ok(NetworkConfigInfo {
        connect_timeout_secs: config.connect_timeout.as_secs(),
        request_timeout_secs: config.request_timeout.as_secs(),
        max_retries: config.max_retries,
        retry_delay_ms: config.retry_delay_ms,
        max_connections: config.max_connections,
        verify_ssl: config.verify_ssl,
    })
}

/// 网络配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfigInfo {
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub max_connections: usize,
    pub verify_ssl: bool,
}

/// 检查环境一致性
#[tauri::command]
pub async fn check_environment_consistency() -> Result<EnvironmentCheckResult> {
    use crate::EnvironmentChecker;
    
    let mut checker = EnvironmentChecker::new();
    let all_passed = checker.run_all_checks();
    
    let checks: Vec<_> = checker.get_checks()
        .iter()
        .map(|check| EnvironmentCheckItem {
            name: check.name.clone(),
            passed: check.passed,
            message: check.message.clone(),
        })
        .collect();
    
    Ok(EnvironmentCheckResult {
        all_passed,
        checks,
    })
}

/// 环境检查项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentCheckItem {
    pub name: String,
    pub passed: bool,
    pub message: String,
}

/// 环境检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentCheckResult {
    pub all_passed: bool,
    pub checks: Vec<EnvironmentCheckItem>,
}

// 辅助函数：生成随机令牌（用于刷新）
#[allow(dead_code)]
fn generate_random_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    
    let mut hasher = RandomState::new().build_hasher();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    hasher.write_u128(timestamp);
    let hash1 = hasher.finish();
    
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(hash1);
    let hash2 = hasher.finish();
    
    format!("{:016x}{:016x}{:016x}{:016x}", hash1, hash2, hash1 ^ hash2, hash2 ^ hash1)
}

// ============================================
// DLP 管理命令
// ============================================

/// 扫描用户输入的敏感信息（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_scan_user_input(
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<SanitizationResult> {
    let dlp = state.dlp_integration.lock().await;
    let result = dlp.scan_user_input(&content).await
        .map_err(|e| Error::DlpError(e.to_string()))?;

    // 异步上报 DLP 扫描事件到 admin backend（不阻塞主流程）
    if result.had_sensitive_data {
        let stats = result.sanitization_stats.clone();
        let was_blocked = result.was_blocked;
        tokio::spawn(async move {
            let admin_url = std::env::var("ADMIN_BACKEND_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());
            let details = if was_blocked {
                format!("DLP 阻止发送: 匹配 {} 处, 阻止 {} 处", stats.total_matches, stats.blocked_count)
            } else {
                format!("DLP 脱敏: 匹配 {} 处, 脱敏 {} 处", stats.total_matches, stats.redacted_count)
            };
            let action = if was_blocked { "dlp_block" } else { "dlp_redact" };
            let _ = reqwest::Client::new()
                .post(format!("{}/api/audit-logs/report", admin_url))
                .json(&serde_json::json!({
                    "action": action,
                    "details": details,
                }))
                .send()
                .await;
        });
    }

    Ok(result)
}

/// 扫描出站请求的敏感信息（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_scan_outbound_request(
    body: String,
    state: tauri::State<'_, CommandState>,
) -> Result<SanitizationResult> {
    let dlp = state.dlp_integration.lock().await;
    dlp.scan_outbound_request(&body).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 为存储脱敏内容（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_sanitize_for_storage(
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<String> {
    let dlp = state.dlp_integration.lock().await;
    dlp.sanitize_for_storage(&content).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 检查HTTP请求是否包含敏感信息（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_check_http_request(
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let dlp = state.dlp_integration.lock().await;
    let body_ref = body.as_deref();
    dlp.check_http_request(&url, &headers, body_ref).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 获取 DLP 配置（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_get_dlp_config(
    state: tauri::State<'_, CommandState>,
) -> Result<DlpIntegrationConfig> {
    let dlp = state.dlp_integration.lock().await;
    Ok(dlp.get_config().await)
}

/// 更新 DLP 配置（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_update_dlp_config(
    config: DlpIntegrationConfig,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let dlp = state.dlp_integration.lock().await;
    dlp.update_config(config).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 获取 DLP 统计信息（旧实现，已被 ipc/dlp.rs 替代）
#[tauri::command]
pub async fn legacy_get_dlp_statistics(
    state: tauri::State<'_, CommandState>,
) -> Result<DlpStatistics> {
    let dlp = state.dlp_integration.lock().await;
    Ok(dlp.get_statistics().await)
}

/// 从后台管理系统同步 DLP 规则（旧实现，已被 ipc/dlp.rs 替代）
///
/// 调用 admin-backend 的 /api/dlp-rules API 获取规则，
/// 然后更新本地 DLP 引擎的自定义规则配置
#[tauri::command]
pub async fn legacy_sync_dlp_rules_from_admin(
    state: tauri::State<'_, CommandState>,
) -> Result<SyncDlpResult> {
    tracing::info!("🔄 Syncing DLP rules from admin backend");

    // 后台管理系统的地址（默认 localhost:3000）
    let admin_url = std::env::var("ADMIN_BACKEND_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| Error::ApiError(format!("Failed to create HTTP client: {}", e)))?;

    // 获取 admin-backend 的 JWT token（如果需要认证）
    let admin_token = std::env::var("ADMIN_AUTH_TOKEN").ok();

    let mut request = client.get(format!("{}/api/dlp-rules", admin_url));
    if let Some(token) = &admin_token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await.map_err(|e| {
        tracing::warn!("⚠️  Failed to connect to admin backend: {}", e);
        Error::ApiError(format!("Admin backend unreachable: {}", e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::warn!("⚠️  Admin backend returned {}: {}", status, body);
        return Err(Error::ApiError(format!(
            "Admin backend returned {}: {}",
            status, body
        )));
    }

    let rules_response: serde_json::Value = response
        .json()
        .await
        .map_err(|e| Error::ApiError(format!("Failed to parse DLP rules: {}", e)))?;

    // 解析规则（支持 {"rules": [...]} 和 [...] 两种格式）
    let rules = if let Some(arr) = rules_response.as_array() {
        arr.clone()
    } else if let Some(arr) = rules_response.get("rules").and_then(|v| v.as_array()) {
        arr.clone()
    } else {
        return Err(Error::ApiError("Expected array of DLP rules".to_string()));
    };

    let mut custom_patterns = Vec::new();
    for rule in &rules {
        let enabled = rule["enabled"].as_bool().unwrap_or(true);
        if !enabled {
            continue;
        }

        let name = rule["name"].as_str().unwrap_or("").to_string();
        let raw_pattern = rule["pattern"].as_str().unwrap_or("").to_string();
        let severity = rule["severity"].as_str().unwrap_or("Medium").to_string();
        let description = rule["description"].as_str().map(|s| s.to_string());
        let replacement = rule["replacement"].as_str().map(|s| s.to_string());
        let rule_type = rule["rule_type"].as_str().unwrap_or("regex");

        if raw_pattern.is_empty() {
            tracing::warn!("Skipping DLP rule '{}': empty pattern", name);
            continue;
        }

        // 根据规则类型构建正则表达式
        let pattern = if rule_type == "keyword" {
            // 关键字规则：从 rule_config 中读取关键字列表，转换为正则
            let rule_config = &rule["rule_config"];
            let keywords: Vec<String> = if let Some(kw_arr) = rule_config.get("keywords").and_then(|v| v.as_array()) {
                kw_arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                // 回退：从 pattern 字段按逗号分割
                raw_pattern.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            };

            if keywords.is_empty() {
                tracing::warn!("Skipping keyword DLP rule '{}': no keywords", name);
                continue;
            }

            let match_mode = rule_config.get("match_mode").and_then(|v| v.as_str()).unwrap_or("contains");
            let case_sensitive = rule_config.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);

            // 转义关键字中的正则特殊字符，然后用 | 连接
            let escaped: Vec<String> = keywords.iter()
                .map(|kw| regex::escape(kw))
                .collect();

            let joined = escaped.join("|");
            let regex_pattern = match match_mode {
                "whole_word" => format!(r"\b(?:{})\b", joined),
                "exact" | "contains" | _ => format!(r"(?:{})", joined),
            };

            // 如果不区分大小写，添加 (?i) 标志
            if !case_sensitive {
                format!("(?i){}", regex_pattern)
            } else {
                regex_pattern
            }
        } else if rule_type == "dictionary" {
            // 字典规则：从 pattern 字段读取逗号分隔的关键字（后端同步时已展开）
            let rule_config = &rule["rule_config"];
            let keywords: Vec<String> = raw_pattern.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if keywords.is_empty() {
                tracing::warn!("Skipping dictionary DLP rule '{}': no keywords in pattern", name);
                continue;
            }

            let match_mode = rule_config.get("match_mode").and_then(|v| v.as_str()).unwrap_or("contains");
            let case_sensitive = rule_config.get("case_sensitive").and_then(|v| v.as_bool()).unwrap_or(false);

            let escaped: Vec<String> = keywords.iter()
                .map(|kw| regex::escape(kw))
                .collect();

            let joined = escaped.join("|");
            let regex_pattern = match match_mode {
                "whole_word" => format!(r"\b(?:{})\b", joined),
                "exact" | "contains" | _ => format!(r"(?:{})", joined),
            };

            if !case_sensitive {
                format!("(?i){}", regex_pattern)
            } else {
                regex_pattern
            }
        } else {
            // 正则表达式规则：处理 /pattern/ 格式
            if raw_pattern.starts_with('/') && raw_pattern.ends_with('/') && raw_pattern.len() > 2 {
                raw_pattern[1..raw_pattern.len() - 1].to_string()
            } else {
                raw_pattern
            }
        };

        // 验证正则表达式是否有效
        if regex::Regex::new(&pattern).is_err() {
            tracing::warn!("Skipping DLP rule '{}': invalid regex pattern '{}'", name, pattern);
            continue;
        }

        // 根据 severity 决定 action
        let action = match severity.as_str() {
            "critical" | "Critical" => "Block",
            "high" | "High" => "Redact",
            "medium" | "Medium" => "Redact",
            "low" | "Low" => "Redact",
            _ => "Redact",
        };

        custom_patterns.push(CustomPatternConfig {
            name,
            pattern,
            severity,
            action: action.to_string(),
            description,
            replacement,
            enabled: true,
        });
    }

    let synced_count = custom_patterns.len();
    tracing::info!("📋 Synced {} DLP rules from admin backend", synced_count);

    // 更新 DLP 配置
    let dlp = state.dlp_integration.lock().await;
    let mut config = dlp.get_config().await;
    config.custom_patterns = custom_patterns;

    dlp.update_config(config)
        .await
        .map_err(|e| Error::DlpError(format!("Failed to update DLP config: {}", e)))?;

    tracing::info!("✅ DLP rules synced successfully");

    Ok(SyncDlpResult {
        synced_count,
        message: format!("Successfully synced {} DLP rules", synced_count),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDlpResult {
    pub synced_count: usize,
    pub message: String,
}

// ============================================================================
// 聊天命令 (Chat Commands)
// ============================================================================

use tauri::{AppHandle, Manager, Window};
use tokio::sync::oneshot;

/// 聊天事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// AI 响应消息
    Response {
        message_id: String,
        content: String,
        thread_id: String,
    },
    /// AI 思考状态
    Thinking {
        message: String,
    },
    /// 状态更新
    Status {
        message: String,
        level: String,
    },
    /// 错误事件
    Error {
        message: String,
        code: Option<String>,
    },
    /// 连接状态
    ConnectionStatus {
        connected: bool,
        message: String,
    },
}

/// SSE 事件订阅管理器
pub struct SseSubscriptionManager {
    /// 是否正在订阅
    is_subscribed: Arc<Mutex<bool>>,
    /// SSE 是否实际连接成功
    is_connected: Arc<Mutex<bool>>,
    /// 取消订阅的通知通道
    cancel_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl SseSubscriptionManager {
    pub fn new() -> Self {
        Self {
            is_subscribed: Arc::new(Mutex::new(false)),
            is_connected: Arc::new(Mutex::new(false)),
            cancel_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// 检查是否正在订阅
    pub async fn is_subscribed(&self) -> bool {
        *self.is_subscribed.lock().await
    }

    /// 检查 SSE 是否实际连接
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.lock().await
    }

    /// 设置订阅状态
    pub async fn set_subscribed(&self, subscribed: bool) {
        *self.is_subscribed.lock().await = subscribed;
    }

    /// 设置连接状态
    pub async fn set_connected(&self, connected: bool) {
        *self.is_connected.lock().await = connected;
    }

    /// 设置取消通道
    pub async fn set_cancel_tx(&self, tx: oneshot::Sender<()>) {
        *self.cancel_tx.lock().await = Some(tx);
    }

    /// 取消订阅
    pub async fn cancel(&self) {
        if let Some(tx) = self.cancel_tx.lock().await.take() {
            let _ = tx.send(());
        }
        self.set_subscribed(false).await;
        self.set_connected(false).await;
    }
}

/// 发送聊天消息
/// 
/// 使用 Tauri IPC 替代 HTTP API,提供类型安全的消息发送
/// 
/// # 参数
/// - `thread_id`: 会话 ID
/// - `content`: 消息内容
/// 
/// # 返回
/// - `SendMessageResponse`: 消息发送响应
/// 
/// # 示例
/// ```typescript
/// const response = await invoke('send_chat_message', {
///   threadId: 'thread-123',
///   content: 'Hello, AI!'
/// });
/// ```
#[tauri::command]
pub async fn legacy_send_chat_message(
    thread_id: String,
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<crate::api_client::SendMessageResponse> {
    tracing::info!("📤 Sending chat message to thread: {}", thread_id);
    tracing::debug!("   Content length: {} chars", content.len());

    // 调用 API 客户端发送消息
    let response = state
        .api_client
        .send_message(crate::api_client::SendMessageRequest {
            thread_id: Some(thread_id.clone()),
            content,
        })
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to send message: {}", e);
            Error::ApiError(e.to_string())
        })?;

    tracing::info!("✅ Message sent successfully: {}", response.message_id);
    Ok(response)
}

/// 订阅聊天事件
/// 
/// 使用 SSE 连接到后端,接收实时消息并通过 Tauri 事件系统推送到前端
/// 
/// # 架构
/// ```
/// 前端 ──invoke──► subscribe_chat_events
///                      │
///                      ▼
///                  SSE 连接
///                      │
///                      ▼
///                  解析事件
///                      │
///                      ▼
///  前端 ◄──emit──  Tauri 事件
/// ```
/// 
/// # 参数
/// - `window`: Tauri 窗口句柄,用于发送事件
/// - `app_handle`: 应用句柄,用于访问全局状态
/// 
/// # 返回
/// - `Ok(())`: 订阅成功启动
/// 
/// # 错误
/// - 如果已经在订阅中,返回错误
/// 
/// # 示例
/// ```typescript
/// await invoke('subscribe_chat_events');
/// 
/// // 监听事件
/// await listen('chat-event', (event) => {
///   console.log('Received:', event.payload);
/// });
/// ```
#[tauri::command]
pub async fn legacy_subscribe_chat_events(
    window: Window,
    app_handle: AppHandle,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    tracing::info!("🔗 Subscribing to chat events");

    // 检查是否已经在订阅
    let manager = app_handle
        .state::<Arc<Mutex<SseSubscriptionManager>>>()
        .inner()
        .clone();

    if manager.lock().await.is_subscribed().await {
        let connected = manager.lock().await.is_connected().await;
        tracing::info!("ℹ️  Already subscribed to chat events, connected={}", connected);
        // 幂等操作：已订阅时重新发送当前连接状态，确保前端状态同步
        let _ = window.emit(
            "chat-event",
            ChatEvent::ConnectionStatus {
                connected,
                message: if connected {
                    "Already connected to chat events".to_string()
                } else {
                    "Reconnecting to chat events...".to_string()
                },
            },
        );
        return Ok(());
    }

    // 创建取消通道
    let (cancel_tx, mut cancel_rx) = oneshot::channel();
    manager.lock().await.set_cancel_tx(cancel_tx).await;
    manager.lock().await.set_subscribed(true).await;

    // 获取 API 客户端
    let api_client = state.api_client.clone();
    let base_url = api_client.base_url().to_string();
    let auth_token = api_client.auth_token().to_string();
    let manager_clone = manager.clone();

    // 在后台任务中订阅 SSE 事件
    tokio::spawn(async move {
        tracing::info!("🚀 Starting SSE subscription task");

        // 构建 SSE URL
        let sse_url = format!("{}/api/chat/events", base_url);
        tracing::debug!("   SSE URL: {}", sse_url);

        // 创建 HTTP 客户端
        let client = reqwest::Client::new();

        loop {
            // 检查是否需要取消
            if cancel_rx.try_recv().is_ok() {
                tracing::info!("🛑 SSE subscription cancelled");
                break;
            }

            // 连接 SSE
            match client
                .get(&sse_url)
                .header("Authorization", format!("Bearer {}", auth_token))
                .header("Accept", "text/event-stream")
                .send()
                .await
            {
                Ok(response) => {
                    // 检查 HTTP 状态码
                    let status = response.status();
                    if !status.is_success() {
                        let body = response.text().await.unwrap_or_default();
                        tracing::error!("❌ SSE connection rejected: {} - {}", status, body);

                        let _ = window.emit(
                            "chat-event",
                            ChatEvent::Error {
                                message: format!(
                                    "SSE connection rejected ({}): {}",
                                    status.as_u16(),
                                    if body.is_empty() { status.canonical_reason().unwrap_or("Unknown error").to_string() } else { body }
                                ),
                                code: Some(format!("HTTP_{}", status.as_u16())),
                            },
                        );

                        // 等待后重试
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        continue;
                    }

                    tracing::info!("✅ SSE connection established (HTTP {})", status.as_u16());

                    // 更新连接状态
                    manager_clone.lock().await.set_connected(true).await;

                    // 发送连接成功事件
                    let _ = window.emit(
                        "chat-event",
                        ChatEvent::ConnectionStatus {
                            connected: true,
                            message: "Connected to chat events".to_string(),
                        },
                    );

                    // 读取 SSE 流
                    let mut stream = response.bytes_stream();
                    use futures::StreamExt;

                    while let Some(chunk) = stream.next().await {
                        // 检查是否需要取消
                        if cancel_rx.try_recv().is_ok() {
                            tracing::info!("🛑 SSE subscription cancelled");
                            break;
                        }

                        match chunk {
                            Ok(bytes) => {
                                // 解析 SSE 事件
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    tracing::debug!("📨 Received SSE data: {}", text);

                                    // 解析事件类型和数据
                                    if let Some(event) = parse_sse_event(&text) {
                                        // 发送事件到前端
                                        if let Err(e) = window.emit("chat-event", event) {
                                            tracing::error!("❌ Failed to emit chat event: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("❌ Error reading SSE stream: {}", e);
                                break;
                            }
                        }
                    }

                    // 连接断开
                    tracing::warn!("⚠️  SSE connection closed");
                    manager_clone.lock().await.set_connected(false).await;
                    let _ = window.emit(
                        "chat-event",
                        ChatEvent::ConnectionStatus {
                            connected: false,
                            message: "Disconnected from chat events".to_string(),
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("❌ Failed to connect to SSE: {}", e);
                    manager_clone.lock().await.set_connected(false).await;

                    // 发送连接失败事件
                    let _ = window.emit(
                        "chat-event",
                        ChatEvent::Error {
                            message: format!("Failed to connect: {}", e),
                            code: Some("CONNECTION_ERROR".to_string()),
                        },
                    );
                }
            }

            // 等待一段时间后重连
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        tracing::info!("🏁 SSE subscription task ended");
    });

    tracing::info!("✅ Chat events subscription started");
    Ok(())
}

/// 取消订阅聊天事件
/// 
/// # 返回
/// - `Ok(())`: 取消订阅成功
/// 
/// # 示例
/// ```typescript
/// await invoke('unsubscribe_chat_events');
/// ```
#[tauri::command]
pub async fn legacy_unsubscribe_chat_events(app_handle: AppHandle) -> Result<()> {
    tracing::info!("🛑 Unsubscribing from chat events");

    let manager = app_handle
        .state::<Arc<Mutex<SseSubscriptionManager>>>()
        .inner()
        .clone();

    manager.lock().await.cancel().await;

    tracing::info!("✅ Chat events unsubscribed");
    Ok(())
}

/// 解析 SSE 事件
/// 
/// SSE 格式:
/// ```
/// event: response
/// data: {"message_id": "123", "content": "Hello"}
/// 
/// ```
/// 
/// # 参数
/// - `text`: SSE 文本数据
/// 
/// # 返回
/// - `Some(ChatEvent)`: 解析成功的事件
/// - `None`: 解析失败或不支持的事件类型
fn parse_sse_event(text: &str) -> Option<ChatEvent> {
    let lines: Vec<&str> = text.lines().collect();

    let mut event_type: Option<&str> = None;
    let mut data: Option<&str> = None;

    for line in lines {
        if line.starts_with("event:") {
            event_type = line.strip_prefix("event:").map(|s| s.trim());
        } else if line.starts_with("data:") {
            data = line.strip_prefix("data:").map(|s| s.trim());
        }
    }

    // 解析事件
    match (event_type, data) {
        (Some("response"), Some(json_data)) => {
            // 解析响应事件
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_data) {
                Some(ChatEvent::Response {
                    message_id: value["message_id"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string(),
                    content: value["content"].as_str().unwrap_or("").to_string(),
                    thread_id: value["thread_id"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string(),
                })
            } else {
                None
            }
        }
        (Some("thinking"), Some(json_data)) => {
            // 解析思考事件
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_data) {
                Some(ChatEvent::Thinking {
                    message: value["message"].as_str().unwrap_or("").to_string(),
                })
            } else {
                None
            }
        }
        (Some("status"), Some(json_data)) => {
            // 解析状态事件
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_data) {
                Some(ChatEvent::Status {
                    message: value["message"].as_str().unwrap_or("").to_string(),
                    level: value["level"].as_str().unwrap_or("info").to_string(),
                })
            } else {
                None
            }
        }
        (Some("error"), Some(json_data)) => {
            // 解析错误事件
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_data) {
                Some(ChatEvent::Error {
                    message: value["message"].as_str().unwrap_or("").to_string(),
                    code: value["code"].as_str().map(|s| s.to_string()),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

// ============================================================================
// 聊天命令测试 (Chat Commands Tests)
// ============================================================================

#[cfg(test)]
mod chat_tests {
    use super::*;

    #[test]
    fn test_parse_sse_response_event() {
        let text = r#"event: response
data: {"message_id": "msg-123", "content": "Hello, world!", "thread_id": "thread-456"}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_some());

        if let Some(ChatEvent::Response {
            message_id,
            content,
            thread_id,
        }) = event
        {
            assert_eq!(message_id, "msg-123");
            assert_eq!(content, "Hello, world!");
            assert_eq!(thread_id, "thread-456");
        } else {
            panic!("Expected Response event");
        }
    }

    #[test]
    fn test_parse_sse_thinking_event() {
        let text = r#"event: thinking
data: {"message": "Processing your request..."}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_some());

        if let Some(ChatEvent::Thinking { message }) = event {
            assert_eq!(message, "Processing your request...");
        } else {
            panic!("Expected Thinking event");
        }
    }

    #[test]
    fn test_parse_sse_status_event() {
        let text = r#"event: status
data: {"message": "Agent started", "level": "info"}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_some());

        if let Some(ChatEvent::Status { message, level }) = event {
            assert_eq!(message, "Agent started");
            assert_eq!(level, "info");
        } else {
            panic!("Expected Status event");
        }
    }

    #[test]
    fn test_parse_sse_error_event() {
        let text = r#"event: error
data: {"message": "Something went wrong", "code": "INTERNAL_ERROR"}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_some());

        if let Some(ChatEvent::Error { message, code }) = event {
            assert_eq!(message, "Something went wrong");
            assert_eq!(code, Some("INTERNAL_ERROR".to_string()));
        } else {
            panic!("Expected Error event");
        }
    }

    #[test]
    fn test_parse_sse_invalid_event() {
        let text = r#"event: unknown
data: {"foo": "bar"}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_sse_malformed_json() {
        let text = r#"event: response
data: {invalid json}

"#;

        let event = parse_sse_event(text);
        assert!(event.is_none());
    }

    #[test]
    fn test_chat_event_serialization() {
        let event = ChatEvent::Response {
            message_id: "msg-123".to_string(),
            content: "Hello".to_string(),
            thread_id: "thread-456".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"response\""));
        assert!(json.contains("\"message_id\":\"msg-123\""));
    }

    #[tokio::test]
    async fn test_sse_subscription_manager() {
        let manager = SseSubscriptionManager::new();

        // 初始状态
        assert!(!manager.is_subscribed().await);
        assert!(!manager.is_connected().await);

        // 设置订阅状态
        manager.set_subscribed(true).await;
        assert!(manager.is_subscribed().await);

        // 设置连接状态
        manager.set_connected(true).await;
        assert!(manager.is_connected().await);

        // 取消订阅（应同时重置连接状态）
        manager.cancel().await;
        assert!(!manager.is_subscribed().await);
        assert!(!manager.is_connected().await);
    }
}
