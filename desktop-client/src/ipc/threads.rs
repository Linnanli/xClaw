//! 线程管理 Tauri Commands。
//!
//! 直接调用 `Database` (ConversationStore) 管理对话线程。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::EngineState;

/// 线程摘要（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: Option<String>,
    pub message_count: i64,
    pub started_at: String,
    pub last_activity: String,
    pub channel: String,
}

/// 对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

/// 列出对话线程。
#[tauri::command]
pub async fn ic_list_threads(
    state: State<'_, EngineState>,
    limit: Option<i64>,
) -> Result<Vec<ThreadSummary>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let conversations = db
        .list_conversations_all_channels(&state.owner_id, limit.unwrap_or(50))
        .await
        .map_err(|e| format!("Failed to list threads: {}", e))?;

    Ok(conversations
        .into_iter()
        .map(|c| ThreadSummary {
            id: c.id.to_string(),
            title: c.title,
            message_count: c.message_count,
            started_at: c.started_at.to_rfc3339(),
            last_activity: c.last_activity.to_rfc3339(),
            channel: c.channel,
        })
        .collect())
}

/// 创建新对话线程。
#[tauri::command]
pub async fn ic_create_thread(state: State<'_, EngineState>) -> Result<String, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let id = db
        .create_conversation("tauri", &state.owner_id, None)
        .await
        .map_err(|e| format!("Failed to create thread: {}", e))?;

    tracing::debug!(thread_id = %id, "Thread created");
    Ok(id.to_string())
}

/// 获取线程消息历史。
#[tauri::command]
pub async fn ic_get_thread_history(
    state: State<'_, EngineState>,
    thread_id: String,
    limit: Option<i64>,
) -> Result<Vec<ThreadMessage>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let uuid = uuid::Uuid::parse_str(&thread_id)
        .map_err(|e| format!("Invalid thread ID: {}", e))?;

    // 权限检查：确认线程属于当前用户
    let belongs = db
        .conversation_belongs_to_user(uuid, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to check thread ownership: {}", e))?;

    if !belongs {
        return Err("Thread not found or access denied".into());
    }

    let (messages, _has_more) = db
        .list_conversation_messages_paginated(uuid, None, limit.unwrap_or(100))
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    Ok(messages
        .into_iter()
        .map(|m| ThreadMessage {
            id: m.id.to_string(),
            role: m.role,
            content: m.content,
            created_at: m.created_at.to_rfc3339(),
        })
        .collect())
}
