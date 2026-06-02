//! Plan Mode & Session Fork Tauri Commands。
//!
//! 通过 `AppState.msg_sender` 将控制信号注入 Agent 消息循环。
//! Plan Mode 切换、Plan 审批/修改、Session Fork 均为控制操作。

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::mpsc;

use ironclaw::channels::IncomingMessage;

use crate::state::EngineState;

// ═══════════════════════════════════════════════════════════════════════
// Response types
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanModeResponse {
    pub thread_id: String,
    pub plan_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanApprovalResponse {
    pub thread_id: String,
    pub plan_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkResponse {
    pub source_thread_id: String,
    pub new_thread_id: String,
    pub at_turn: u64,
}

// ═══════════════════════════════════════════════════════════════════════
// Toggle Plan Mode
// ═══════════════════════════════════════════════════════════════════════

/// 切换线程的 Plan Mode（规划/执行模式）。
///
/// 发送 `/plan-mode` 控制命令到 Agent 消息循环，
/// Agent 收到后切换 `Thread.plan_mode` 状态。
///
/// Plan Mode 下：
/// - LLM 仅可调用只读工具
/// - 写入工具返回 dry-run 预览
/// - Agent 输出结构化 Plan
#[tauri::command]
pub async fn ic_toggle_plan_mode(
    state: State<'_, EngineState>,
    thread_id: String,
) -> Result<PlanModeResponse, String> {
    let state = state.get()?;
    dispatch_control_message(
        &state.msg_sender,
        &state.scope_id,
        &thread_id,
        "/plan-mode",
        "toggle plan mode",
    )
    .await?;

    Ok(PlanModeResponse {
        thread_id,
        plan_mode: true, // 实际状态由 Agent 异步更新，前端通过事件获取
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Approve Plan
// ═══════════════════════════════════════════════════════════════════════

/// 批准当前 Plan 并开始执行。
///
/// 发送 `/approve-plan {plan_id}` 控制命令，
/// Agent 将 Plan 步骤逐一执行。
#[tauri::command]
pub async fn ic_approve_plan(
    state: State<'_, EngineState>,
    thread_id: String,
    plan_id: String,
) -> Result<PlanApprovalResponse, String> {
    let state = state.get()?;

    let content = format!("/approve-plan {plan_id}");
    dispatch_control_message(
        &state.msg_sender,
        &state.scope_id,
        &thread_id,
        &content,
        "approve plan",
    )
    .await?;

    Ok(PlanApprovalResponse {
        thread_id,
        plan_id,
        status: "approved".into(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Revise Plan
// ═══════════════════════════════════════════════════════════════════════

/// 要求 Agent 根据反馈修改 Plan。
///
/// 发送 `/revise-plan {plan_id} {feedback}` 控制命令。
#[tauri::command]
pub async fn ic_revise_plan(
    state: State<'_, EngineState>,
    thread_id: String,
    plan_id: String,
    feedback: String,
) -> Result<PlanApprovalResponse, String> {
    let state = state.get()?;

    let content = format!("/revise-plan {plan_id} {feedback}");
    dispatch_control_message(
        &state.msg_sender,
        &state.scope_id,
        &thread_id,
        &content,
        "revise plan",
    )
    .await?;

    Ok(PlanApprovalResponse {
        thread_id,
        plan_id,
        status: "revision_requested".into(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Fork Thread
// ═══════════════════════════════════════════════════════════════════════

/// 从指定 Turn 创建会话分支。
///
/// 发送 `/fork {at_turn}` 控制命令到 Agent。
/// Agent 调用 `Session::fork_thread()` 创建新 Thread。
/// 前端通过 `chat-stream` 接收 fork 完成事件。
#[tauri::command]
pub async fn ic_fork_thread(
    state: State<'_, EngineState>,
    thread_id: String,
    at_turn: u64,
) -> Result<ForkResponse, String> {
    let state = state.get()?;

    let content = format!("/fork {at_turn}");
    dispatch_control_message(
        &state.msg_sender,
        &state.scope_id,
        &thread_id,
        &content,
        "fork thread",
    )
    .await?;

    // 实际的 new_thread_id 由 Agent 异步通过 chat-stream 返回
    Ok(ForkResponse {
        source_thread_id: thread_id,
        new_thread_id: String::new(),
        at_turn,
    })
}

fn build_control_message(scope_id: &str, thread_id: &str, content: &str) -> IncomingMessage {
    IncomingMessage::new("tauri", scope_id, content)
        .with_thread(thread_id)
        .with_owner_id(scope_id)
}

async fn dispatch_control_message(
    sender: &mpsc::Sender<IncomingMessage>,
    scope_id: &str,
    thread_id: &str,
    content: &str,
    operation: &str,
) -> Result<(), String> {
    sender
        .send(build_control_message(scope_id, thread_id, content))
        .await
        .map_err(|e| format!("Failed to {operation}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_control_message(msg: IncomingMessage, content: &str) {
        assert_eq!(msg.channel, "tauri");
        assert_eq!(msg.user_id, "owner-1");
        assert_eq!(msg.owner_id, "owner-1");
        assert_eq!(msg.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(msg.conversation_scope(), Some("thread-a"));
        assert_eq!(msg.content, content);
    }

    #[tokio::test]
    async fn req_plan_mode_i1_control_sequence_preserves_thread_scope() {
        let (tx, mut rx) = mpsc::channel(4);
        for content in [
            "/plan-mode".to_string(),
            "/approve-plan plan-1".to_string(),
            "/revise-plan plan-1 add more tests".to_string(),
            "/fork 2".to_string(),
        ] {
            dispatch_control_message(&tx, "owner-1", "thread-a", &content, "test")
                .await
                .expect("control message should dispatch");
            let msg = rx.recv().await.expect("message should be queued");
            assert_control_message(msg, &content);
        }
    }

    #[test]
    fn req_plan_mode_i1_response_contracts_are_stable() {
        let approval = PlanApprovalResponse {
            thread_id: "thread-a".to_string(),
            plan_id: "plan-1".to_string(),
            status: "approved".to_string(),
        };
        let fork = ForkResponse {
            source_thread_id: "thread-a".to_string(),
            new_thread_id: String::new(),
            at_turn: 2,
        };

        let approval_json = serde_json::to_value(approval).expect("approval serializes");
        assert_eq!(approval_json["status"], "approved");
        assert_eq!(approval_json["thread_id"], "thread-a");

        let fork_json = serde_json::to_value(fork).expect("fork serializes");
        assert_eq!(fork_json["source_thread_id"], "thread-a");
        assert_eq!(fork_json["new_thread_id"], "");
        assert_eq!(fork_json["at_turn"], 2);
    }
}
