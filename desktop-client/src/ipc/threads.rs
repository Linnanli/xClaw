//! 线程管理 Tauri Commands。
//!
//! 直接调用 `Database` (ConversationStore) 管理对话线程。

use serde::{Deserialize, Serialize};
use tauri::State;

use ironclaw::history::PersistedAttachment;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ThreadAttachment>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadAttachment {
    pub id: String,
    #[serde(rename = "type")]
    pub attachment_type: String,
    pub name: String,
    #[serde(rename = "contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    pub status: ThreadAttachmentStatus,
    pub content: Vec<ThreadAttachmentPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadAttachmentStatus {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThreadAttachmentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        image: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
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
        .list_conversations_all_channels(&state.scope_id, limit.unwrap_or(50))
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
        .create_conversation("tauri", &state.scope_id, None)
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

    let uuid =
        uuid::Uuid::parse_str(&thread_id).map_err(|e| format!("Invalid thread ID: {}", e))?;

    // 权限检查：确认线程属于当前用户
    let belongs = db
        .conversation_belongs_to_user(uuid, &state.scope_id)
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
            attachments: map_thread_attachments(&m.attachments),
            created_at: m.created_at.to_rfc3339(),
        })
        .collect())
}

fn map_thread_attachments(attachments: &[PersistedAttachment]) -> Vec<ThreadAttachment> {
    attachments
        .iter()
        .map(|attachment| ThreadAttachment {
            id: attachment.id.clone(),
            attachment_type: map_attachment_type(&attachment.kind).to_string(),
            name: attachment
                .filename
                .clone()
                .unwrap_or_else(|| fallback_attachment_name(attachment)),
            content_type: Some(attachment.mime_type.clone()),
            status: ThreadAttachmentStatus {
                kind: "complete".to_string(),
            },
            content: build_thread_attachment_content(attachment),
        })
        .collect()
}

fn map_attachment_type(kind: &str) -> &'static str {
    match kind {
        "image" => "image",
        "document" => "document",
        _ => "file",
    }
}

fn fallback_attachment_name(attachment: &PersistedAttachment) -> String {
    if attachment.kind == "image" {
        "image".to_string()
    } else if attachment.kind == "audio" {
        "audio".to_string()
    } else {
        "attachment".to_string()
    }
}

fn build_thread_attachment_content(attachment: &PersistedAttachment) -> Vec<ThreadAttachmentPart> {
    if let Some(image) = attachment.image_data_url() {
        return vec![ThreadAttachmentPart::Image {
            image,
            filename: attachment.filename.clone(),
        }];
    }

    vec![ThreadAttachmentPart::Text {
        text: attachment.extracted_text.clone().unwrap_or_default(),
    }]
}
