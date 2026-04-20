//! 工具审批 Tauri Commands。
//!
//! 通过消息系统处理工具审批请求。
//! IronClaw 的审批机制是基于消息的：发送 JSON 格式的 `ExecApproval`，
//! Agent 的 SubmissionParser 解析后执行审批操作。

use ironclaw::agent::Submission;
use ironclaw::channels::IncomingMessage;
use tauri::State;
use uuid::Uuid;

use crate::state::EngineState;

/// 构建 ExecApproval JSON 消息体，供 SubmissionParser 解析为 `Submission::ExecApproval`。
fn build_exec_approval_json(request_id: &str, approved: bool) -> Result<String, String> {
    let uuid = Uuid::parse_str(request_id)
        .map_err(|e| format!("Invalid request_id UUID: {}", e))?;
    let submission = Submission::ExecApproval {
        request_id: uuid,
        approved,
        always: false,
    };
    serde_json::to_string(&submission)
        .map_err(|e| format!("Failed to serialize approval: {}", e))
}

/// 审批工具执行请求。
///
/// 通过向 Agent 发送 JSON 格式的 ExecApproval 消息来处理。
/// SubmissionParser 解析 JSON 后匹配 `Submission::ExecApproval`，绕过 AwaitingApproval 状态检查。
#[tauri::command]
pub async fn ic_approve_tool(
    state: State<'_, EngineState>,
    request_id: String,
    thread_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let content = build_exec_approval_json(&request_id, true)?;

    let msg = IncomingMessage::new("tauri", &state.scope_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.scope_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send approval: {}", e))?;

    tracing::debug!(request_id = %request_id, "Tool approved via ExecApproval");
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
    let content = build_exec_approval_json(&request_id, false)?;

    let msg = IncomingMessage::new("tauri", &state.scope_id, &content)
        .with_thread(&thread_id)
        .with_owner_id(&state.scope_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send denial: {}", e))?;

    tracing::debug!(request_id = %request_id, "Tool denied via ExecApproval");
    Ok(())
}
