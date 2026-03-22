//! 聊天相关 Tauri Commands。
//!
//! 通过 `AppState.msg_sender` 将用户消息注入 TauriChannel，
//! Agent 处理后通过 `ChatEvent` 推送回复到前端。
//!
//! # 前端兼容性
//!
//! 命令名与现有前端 `useAiChatTauri.ts` 完全匹配：
//! - `send_chat_message` — 发送消息
//! - `subscribe_chat_events` — 订阅事件（新架构下为 no-op）
//! - `unsubscribe_chat_events` — 取消订阅（新架构下为 no-op）

use ironclaw::channels::IncomingMessage;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 发送消息的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub success: bool,
}

/// 发送聊天消息。
///
/// 构造 `IncomingMessage` 并通过 `msg_sender` 注入 Agent 消息循环。
/// AI 回复通过 `chat-event` Tauri 事件异步推送到前端。
///
/// # 参数
///
/// - `thread_id`: 对话线程 ID
/// - `content`: 消息内容（应已经过前端 DLP 扫描）
///
/// # 安全
///
/// - 消息内容不写入日志（防止敏感信息泄露）
/// - 仅记录 message_id 和 thread_id 用于追踪
#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, AppState>,
    thread_id: String,
    content: String,
) -> Result<SendMessageResponse, String> {
    let message_id = uuid::Uuid::new_v4().to_string();

    let msg = IncomingMessage::new("tauri", &state.owner_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.owner_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send message to agent: {}", e))?;

    tracing::debug!(
        message_id = %message_id,
        thread_id = %thread_id,
        "Message injected into agent loop"
    );

    Ok(SendMessageResponse {
        message_id,
        success: true,
    })
}

/// 订阅聊天事件（兼容命令）。
///
/// 新架构下 TauriChannel 在引擎启动时自动推送事件到前端，
/// 前端的 `listen('chat-event')` 天然就是订阅。
/// 此命令保留仅为兼容现有前端代码，实际为 no-op。
#[tauri::command]
pub async fn subscribe_chat_events() -> Result<(), String> {
    tracing::debug!("subscribe_chat_events called (no-op in embedded mode)");
    Ok(())
}

/// 取消订阅聊天事件（兼容命令）。
///
/// 新架构下无需手动取消订阅，前端 `unlisten()` 即可停止接收事件。
/// 此命令保留仅为兼容现有前端代码，实际为 no-op。
#[tauri::command]
pub async fn unsubscribe_chat_events() -> Result<(), String> {
    tracing::debug!("unsubscribe_chat_events called (no-op in embedded mode)");
    Ok(())
}
