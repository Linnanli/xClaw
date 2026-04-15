//! 任务管理 Tauri Commands。
//!
//! 直接调用 IronClaw 的 `Database` 和 `Scheduler` 组件，
//! 提供任务事件历史和后续提示功能。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::state::EngineState;
use crate::tauri_channel::ChatEvent;
use ironclaw::context::JobState;

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
    /// 关联的对话 thread ID，用于前端点击任务时跳转到对应对话
    pub conversation_id: Option<String>,
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

/// 任务详情（点击任务卡片时加载）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetailResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub conversation_id: Option<String>,
    /// 失败原因（手动/定时/事件任务失败时存在）
    pub failure_reason: Option<String>,
    /// Token 消耗（agent 任务）
    pub total_tokens_used: Option<u64>,
    /// 事件历史（手动/定时/事件均覆盖）
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

fn ensure_owned_job(
    state: &crate::state::AppState,
    job: &ironclaw::context::JobContext,
) -> Result<(), String> {
    if job.user_id == state.scope_id {
        return Ok(());
    }

    Err("Job not found or access denied".into())
}

async fn stop_active_job(
    scheduler: Option<std::sync::Arc<ironclaw::agent::Scheduler>>,
    job_id: Uuid,
    job_state: JobState,
) -> Result<(), String> {
    if !job_state.is_active() {
        return Ok(());
    }

    let scheduler = scheduler
        .ok_or_else(|| "Failed to cancel job: active job scheduler unavailable".to_string())?;

    scheduler
        .stop(job_id)
        .await
        .map_err(|error| format!("Failed to cancel job: {}", error))
}

fn emit_job_status(app_handle: &AppHandle, job_id: Uuid, title: &str, status: &str) {
    let event = ChatEvent::JobStatus {
        job_id: job_id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
    };
    let _ = app_handle.emit("chat-event", event);
}

fn restart_job_title(job: &ironclaw::context::JobContext, failure_reason: &str) -> String {
    if failure_reason.is_empty() || matches!(job.state, JobState::Cancelled) {
        return job.title.clone();
    }

    format!("Previous attempt failed: {}. Retry: {}", failure_reason, job.title)
}

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 列出当前用户的所有任务。
#[tauri::command]
pub async fn ic_list_jobs(state: State<'_, EngineState>) -> Result<Vec<JobInfoResponse>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?.clone();

    let jobs = db
        .list_agent_jobs_for_user(&state.scope_id)
        .await
        .map_err(|e| format!("Failed to list jobs: {}", e))?;

    let mut tasks = tokio::task::JoinSet::new();
    for (idx, j) in jobs.into_iter().enumerate() {
        let db = db.clone();
        tasks.spawn(async move {
            let conversation_id = db
                .get_job(j.id)
                .await
                .ok()
                .flatten()
                .and_then(|ctx| ctx.conversation_id)
                .map(|id| id.to_string());
            (
                idx,
                JobInfoResponse {
                    id: j.id.to_string(),
                    title: j.title,
                    status: j.status,
                    created_at: j.created_at.to_rfc3339(),
                    started_at: j.started_at.map(|t| t.to_rfc3339()),
                    completed_at: j.completed_at.map(|t| t.to_rfc3339()),
                    conversation_id,
                },
            )
        });
    }

    let mut ordered: Vec<Option<JobInfoResponse>> = vec![None; tasks.len()];
    while let Some(res) = tasks.join_next().await {
        let (idx, item) = res.map_err(|e| format!("Task join error: {e}"))?;
        if idx < ordered.len() {
            ordered[idx] = Some(item);
        }
    }
    Ok(ordered.into_iter().flatten().collect())
}

/// 获取任务详情（含运行结果、失败原因、事件历史）。
///
/// 聚合多个数据库查询，供前端任务详情视图使用。
/// 适用于手动、定时、事件三种任务类型。
#[tauri::command]
pub async fn ic_get_job_detail(
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<JobDetailResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

    let ctx = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;

    let failure_reason = if ctx.state == JobState::Failed {
        db.get_agent_job_failure_reason(uuid).await.unwrap_or(None)
    } else {
        None
    };

    let events = db
        .list_job_events(uuid, None)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|e| JobEvent {
            id: e.id,
            event_type: e.event_type,
            data: e.data,
            created_at: e.created_at.to_rfc3339(),
        })
        .collect();

    Ok(JobDetailResponse {
        id: job_id,
        title: ctx.title,
        description: ctx.description,
        status: ctx.state.to_string(),
        source: "direct".to_string(),
        created_at: ctx.created_at.to_rfc3339(),
        started_at: ctx.started_at.map(|t| t.to_rfc3339()),
        completed_at: ctx.completed_at.map(|t| t.to_rfc3339()),
        conversation_id: ctx.conversation_id.map(|id| id.to_string()),
        failure_reason,
        total_tokens_used: (ctx.total_tokens_used > 0).then_some(ctx.total_tokens_used),
        events,
    })
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
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

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
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    // 通过消息系统发送后续提示
    let prompt_content = format!("!prompt {} {}", uuid, content);

    let msg = ironclaw::channels::IncomingMessage::new("tauri", &state.scope_id, &prompt_content)
        .with_owner_id(&state.scope_id);

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

/// 取消运行中的 agent 任务。
#[tauri::command]
pub async fn ic_cancel_job(
    app_handle: tauri::AppHandle,
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let job = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;

    ensure_owned_job(state, &job)?;

    let scheduler = state.scheduler_slot.read().await.as_ref().cloned();
    stop_active_job(scheduler, uuid, job.state).await?;

    db.update_job_status(uuid, JobState::Cancelled, Some("Cancelled by user"))
        .await
        .map_err(|e| format!("Failed to cancel job: {}", e))?;

    emit_job_status(&app_handle, uuid, &job.title, "cancelled");
    tracing::info!(job_id = %uuid, "Job cancelled");
    Ok(())
}

#[cfg(test)]
mod cancel_tests {
    use super::stop_active_job;
    use ironclaw::context::JobState;
    use uuid::Uuid;

    #[tokio::test]
    async fn active_job_without_scheduler_is_rejected() {
        let error = stop_active_job(None, Uuid::nil(), JobState::InProgress)
            .await
            .expect_err("active jobs require scheduler stop before marking cancelled");

        assert_eq!(error, "Failed to cancel job: active job scheduler unavailable");
    }

    #[tokio::test]
    async fn inactive_job_without_scheduler_is_allowed() {
        stop_active_job(None, Uuid::nil(), JobState::Completed)
            .await
            .expect("inactive job should not require scheduler stop");
    }
}

/// 重试一个已结束的 agent 任务。
#[tauri::command]
pub async fn ic_restart_job(
    app_handle: tauri::AppHandle,
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let job = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;

    ensure_owned_job(state, &job)?;

    if job.state.is_active() {
        return Err(format!("Cannot restart active job in state '{}'", job.state));
    }

    let scheduler = state
        .scheduler_slot
        .read()
        .await
        .as_ref()
        .cloned()
        .ok_or("Scheduler not available")?;

    let failure_reason = db
        .get_agent_job_failure_reason(uuid)
        .await
        .map_err(|e| format!("Failed to load job failure reason: {}", e))?
        .unwrap_or_default();

    let title = restart_job_title(&job, &failure_reason);
    let new_job_id = scheduler
        .dispatch_job(&job.user_id, &title, &job.description, None)
        .await
        .map_err(|e| format!("Failed to restart job: {}", e))?;

    emit_job_status(&app_handle, new_job_id, &title, "in_progress");
    tracing::info!(old_job_id = %uuid, new_job_id = %new_job_id, "Job restarted");
    Ok(())
}
