//! 任务管理 Tauri Commands。
//!
//! 直接调用 IronClaw 的 `Database` 和 `Scheduler` 组件，
//! 提供任务事件历史和后续提示功能。

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::state::EngineState;

// ─── 数据类型 ────────────────────────────────────────────────────────

/// 任务概要（前端列表展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfoResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// 任务事件（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub id: i64,
    pub event_type: String,
    pub data: serde_json::Value,
    pub created_at: String,
}

/// 任务事件列表响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEventsResponse {
    pub job_id: String,
    pub events: Vec<JobEvent>,
}

/// 后续提示请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPromptRequest {
    pub content: String,
    #[serde(default)]
    pub done: bool,
}

/// 后续提示响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPromptResponse {
    pub status: String,
    pub job_id: String,
}

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 列出当前用户的所有任务。
#[tauri::command]
pub async fn ic_list_jobs(
    state: State<'_, EngineState>,
) -> Result<Vec<JobInfoResponse>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let jobs = db
        .list_agent_jobs_for_user(&state.owner_id)
        .await
        .map_err(|e| format!("Failed to list jobs: {}", e))?;

    Ok(jobs
        .into_iter()
        .map(|j| JobInfoResponse {
            id: j.id.to_string(),
            title: j.title,
            status: j.status,
            created_at: j.created_at.to_rfc3339(),
            started_at: j.started_at.map(|t| t.to_rfc3339()),
            completed_at: j.completed_at.map(|t| t.to_rfc3339()),
        })
        .collect())
}

/// 获取任务事件历史。
///
/// 从数据库加载持久化的任务事件，用于页面打开时的历史回放。
#[tauri::command]
pub async fn ic_job_events(
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<JobEventsResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&job_id)
        .map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state
        .db
        .as_ref()
        .ok_or("Database not available")?;

    let events = db
        .list_job_events(uuid, None)
        .await
        .map_err(|e| format!("Failed to load job events: {}", e))?;

    let items: Vec<JobEvent> = events
        .into_iter()
        .map(|e| JobEvent {
            id: e.id,
            event_type: e.event_type,
            data: e.data,
            created_at: e.created_at.to_rfc3339(),
        })
        .collect();

    tracing::debug!(job_id = %job_id, count = items.len(), "Job events loaded");

    Ok(JobEventsResponse {
        job_id,
        events: items,
    })
}

/// 向运行中的任务发送后续提示。
///
/// 支持两种任务类型：
/// - Agent 任务 → 通过消息系统注入
/// - Sandbox 任务 → 不支持（需要 scheduler，当前客户端无直接访问）
///
/// 客户端侧通过消息系统实现：发送格式化的 prompt 消息到对应线程。
#[tauri::command]
pub async fn ic_job_prompt(
    state: State<'_, EngineState>,
    job_id: String,
    content: String,
) -> Result<JobPromptResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&job_id)
        .map_err(|_| format!("Invalid job ID: {}", job_id))?;

    // 通过消息系统发送后续提示
    let prompt_content = format!("!prompt {} {}", uuid, content);

    let msg = ironclaw::channels::IncomingMessage::new(
        "tauri",
        &state.owner_id,
        &prompt_content,
    )
    .with_owner_id(&state.owner_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send prompt: {}", e))?;

    tracing::info!(job_id = %job_id, "Follow-up prompt sent");

    Ok(JobPromptResponse {
        status: "sent".to_string(),
        job_id,
    })
}
