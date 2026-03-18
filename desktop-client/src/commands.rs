use crate::auth::AuthManager;
use crate::storage::StorageManager;
use crate::extension_manager::ExtensionManager;
use crate::routine_manager::RoutineManager;
use crate::memory_manager::{is_protected_file, DeleteResult, MemoryApiExtensions};
use crate::dlp::{DlpIntegration, DlpIntegrationConfig, DlpStatistics, SanitizationResult};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;

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
        // Default to localhost:3000 for development (Web Gateway)
        let api_client = crate::api_client::ApiClient::new("http://localhost:3000".to_string());
        
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
            "http://localhost:3000".to_string(),
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

/// 扫描用户输入的敏感信息
#[tauri::command]
pub async fn scan_user_input(
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<SanitizationResult> {
    let dlp = state.dlp_integration.lock().await;
    dlp.scan_user_input(&content).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 扫描出站请求的敏感信息
#[tauri::command]
pub async fn scan_outbound_request(
    body: String,
    state: tauri::State<'_, CommandState>,
) -> Result<SanitizationResult> {
    let dlp = state.dlp_integration.lock().await;
    dlp.scan_outbound_request(&body).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 为存储脱敏内容
#[tauri::command]
pub async fn sanitize_for_storage(
    content: String,
    state: tauri::State<'_, CommandState>,
) -> Result<String> {
    let dlp = state.dlp_integration.lock().await;
    dlp.sanitize_for_storage(&content).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 检查HTTP请求是否包含敏感信息
#[tauri::command]
pub async fn check_http_request(
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

/// 获取 DLP 配置
#[tauri::command]
pub async fn get_dlp_config(
    state: tauri::State<'_, CommandState>,
) -> Result<DlpIntegrationConfig> {
    let dlp = state.dlp_integration.lock().await;
    Ok(dlp.get_config().await)
}

/// 更新 DLP 配置
#[tauri::command]
pub async fn update_dlp_config(
    config: DlpIntegrationConfig,
    state: tauri::State<'_, CommandState>,
) -> Result<()> {
    let dlp = state.dlp_integration.lock().await;
    dlp.update_config(config).await
        .map_err(|e| Error::DlpError(e.to_string()))
}

/// 获取 DLP 统计信息
#[tauri::command]
pub async fn get_dlp_statistics(
    state: tauri::State<'_, CommandState>,
) -> Result<DlpStatistics> {
    let dlp = state.dlp_integration.lock().await;
    Ok(dlp.get_statistics().await)
}
