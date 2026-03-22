//! 工具审批 Tauri Commands。
//!
//! 通过消息系统处理工具审批请求。
//! IronClaw 的审批机制是基于消息的：用户发送 "approve" 或 "deny" 消息，
//! Agent 的 SubmissionParser 解析后执行审批操作。

use ironclaw::channels::IncomingMessage;
use tauri::State;

use crate::state::EngineState;

/// 审批工具执行请求。
///
/// 通过向 Agent 发送格式化的审批消息来处理。
/// Agent 的 SubmissionParser 会识别 `!approve <request_id>` 格式。
#[tauri::command]
pub async fn ic_approve_tool(
    state: State<'_, EngineState>,
    request_id: String,
    thread_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let content = format!("!approve {}", request_id);

    let msg = IncomingMessage::new("tauri", &state.owner_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.owner_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send approval: {}", e))?;

    tracing::debug!(request_id = %request_id, "Tool approved");
    Ok(())
}

/// 拒绝工具执行请求。
#[tauri::command]
pub async fn ic_deny_tool(
    state: State<'_, EngineState>,
    request_id: String,
    thread_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let content = format!("!deny {}", request_id);

    let msg = IncomingMessage::new("tauri", &state.owner_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.owner_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send denial: {}", e))?;

    tracing::debug!(request_id = %request_id, "Tool denied");
    Ok(())
}
