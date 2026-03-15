use crate::auth::AuthManager;
use crate::storage::StorageManager;
use crate::extension_manager::ExtensionManager;
use crate::routine_manager::RoutineManager;
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
    pub api_client: Arc<crate::api_client::ApiClient>,
}

impl CommandState {
    pub fn new() -> Self {
        // Default to localhost:8000 for development
        let api_client = crate::api_client::ApiClient::new("http://localhost:8000".to_string());
        
        Self {
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            storage_manager: Arc::new(Mutex::new(None)),
            extension_manager: Arc::new(StdMutex::new(ExtensionManager::new())),
            routine_manager: Arc::new(StdMutex::new(RoutineManager::new())),
            api_client: Arc::new(api_client),
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
) -> Result<crate::api_client::MemoryContent> {
    state.api_client.write_memory(&memory_id, &content).await
}

#[tauri::command]
pub async fn search_memory(
    query: String,
    state: tauri::State<'_, CommandState>,
) -> Result<Vec<crate::api_client::MemoryContent>> {
    state.api_client.search_memory(&query).await
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
