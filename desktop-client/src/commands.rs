use crate::auth::AuthManager;
use crate::storage::StorageManager;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
}

impl CommandState {
    pub fn new() -> Self {
        Self {
            auth_manager: Arc::new(Mutex::new(AuthManager::new())),
            storage_manager: Arc::new(Mutex::new(None)),
        }
    }
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
