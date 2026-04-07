//! 日程管理 Tauri Commands。
//!
//! 直接调用 IronClaw 的 `Database` 组件（`RoutineStore` trait）管理定时任务。

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use ironclaw::agent::routine::{Routine, RoutineAction, RoutineGuardrails, Trigger};
use ironclaw::agent::routine::next_cron_fire;
use crate::state::EngineState;

// ─── 数据类型 ────────────────────────────────────────────────────────

/// 日程执行记录（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRun {
    pub id: String,
    pub trigger_type: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub result_summary: Option<String>,
    pub tokens_used: Option<i32>,
    pub job_id: Option<String>,
}

/// 日程执行历史响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRunsResponse {
    pub routine_id: String,
    pub runs: Vec<RoutineRun>,
}

/// 前端展示用的日程信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub trigger: serde_json::Value,
}

/// 创建日程的请求体。
#[derive(Debug, Deserialize)]
pub struct CreateRoutineRequest {
    pub name: String,
    pub description: String,
    /// trigger JSON，格式与 ironclaw Trigger 的 serde 表示一致
    pub trigger: serde_json::Value,
    /// 任务执行时发给 LLM 的提示词
    #[serde(default)]
    pub prompt: String,
}

// ─── 辅助函数 ────────────────────────────────────────────────────────

fn routine_to_info(r: &Routine) -> RoutineInfo {
    let status = if r.enabled { "active" } else { "inactive" }.to_string();
    let trigger = serde_json::to_value(&r.trigger).unwrap_or(serde_json::Value::Null);
    RoutineInfo {
        id: r.id.to_string(),
        name: r.name.clone(),
        description: r.description.clone(),
        status,
        trigger,
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 列出所有日程。
#[tauri::command]
pub async fn ic_list_routines(
    state: State<'_, EngineState>,
) -> Result<Vec<RoutineInfo>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;
    let routines = db
        .list_routines(&state.owner_id)
        .await
        .map_err(|e| format!("Failed to list routines: {}", e))?;
    Ok(routines.iter().map(routine_to_info).collect())
}

/// 创建新日程。
#[tauri::command]
pub async fn ic_create_routine(
    state: State<'_, EngineState>,
    request: CreateRoutineRequest,
) -> Result<RoutineInfo, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let trigger: Trigger = serde_json::from_value(request.trigger)
        .map_err(|e| format!("Invalid trigger format: {}", e))?;

    // 对 cron 触发器：若前端未指定 timezone，自动填充系统本地时区，
    // 确保用户输入的时间按本地时间解释而非 UTC。
    let trigger = match trigger {
        Trigger::Cron { schedule, timezone: None } => {
            let local_tz = ironclaw::timezone::detect_system_timezone().to_string();
            Trigger::Cron { schedule, timezone: Some(local_tz) }
        }
        other => other,
    };

    // 对 cron 触发器计算初始 next_fire_at，否则引擎永远不会调度该任务
    let next_fire_at = match &trigger {
        Trigger::Cron { schedule, timezone } => {
            next_cron_fire(schedule, timezone.as_deref())
                .map_err(|e| format!("Invalid cron expression: {}", e))?
        }
        _ => None,
    };

    let now = chrono::Utc::now();
    let routine = Routine {
        id: Uuid::new_v4(),
        name: request.name,
        description: request.description,
        user_id: state.owner_id.clone(),
        enabled: true,
        trigger,
        action: RoutineAction::Lightweight {
            prompt: request.prompt,
            context_paths: Vec::new(),
            max_tokens: 4096,
            use_tools: false,
            max_tool_rounds: 3,
        },
        guardrails: RoutineGuardrails::default(),
        notify: ironclaw::agent::routine::NotifyConfig::default(),
        last_run_at: None,
        next_fire_at,
        run_count: 0,
        consecutive_failures: 0,
        state: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    };

    db.create_routine(&routine)
        .await
        .map_err(|e| format!("Failed to create routine: {}", e))?;

    tracing::info!(id = %routine.id, name = %routine.name, "Routine created");
    Ok(routine_to_info(&routine))
}

/// 启用或禁用日程。
#[tauri::command]
pub async fn ic_toggle_routine(
    state: State<'_, EngineState>,
    routine_id: String,
    enabled: bool,
) -> Result<(), String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;
    let uuid = Uuid::parse_str(&routine_id)
        .map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let mut routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;

    routine.enabled = enabled;
    routine.updated_at = chrono::Utc::now();

    db.update_routine(&routine)
        .await
        .map_err(|e| format!("Failed to update routine: {}", e))?;

    tracing::debug!(id = %routine_id, enabled, "Routine toggled");
    Ok(())
}

/// 删除日程。
#[tauri::command]
pub async fn ic_delete_routine(
    state: State<'_, EngineState>,
    routine_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;
    let uuid = Uuid::parse_str(&routine_id)
        .map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    db.delete_routine(uuid)
        .await
        .map_err(|e| format!("Failed to delete routine: {}", e))?;

    tracing::info!(id = %routine_id, "Routine deleted");
    Ok(())
}

/// 获取日程执行历史。
#[tauri::command]
pub async fn ic_routine_runs(
    state: State<'_, EngineState>,
    routine_id: String,
) -> Result<RoutineRunsResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&routine_id)
        .map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

    let runs = db
        .list_routine_runs(uuid, 50)
        .await
        .map_err(|e| format!("Failed to load routine runs: {}", e))?;

    let items = runs
        .iter()
        .map(|run| RoutineRun {
            id: run.id.to_string(),
            trigger_type: run.trigger_type.clone(),
            started_at: run.started_at.to_rfc3339(),
            completed_at: run.completed_at.map(|dt| dt.to_rfc3339()),
            status: format!("{:?}", run.status),
            result_summary: run.result_summary.clone(),
            tokens_used: run.tokens_used,
            job_id: run.job_id.map(|id| id.to_string()),
        })
        .collect();

    Ok(RoutineRunsResponse { routine_id, runs: items })
}

/// 手动触发日程立即执行的响应。
#[derive(Debug, Serialize)]
pub struct FireRoutineResponse {
    pub thread_id: String,
    pub prompt: String,
}

/// 手动触发日程立即执行。
///
/// 返回 thread_id 和 prompt，前端先跳转到对话页面，
/// 再通过 send_chat_message 发送提示词，确保 thinking/streaming 事件不会被错过。
#[tauri::command]
pub async fn ic_fire_routine(
    state: State<'_, crate::state::EngineState>,
    routine_id: String,
) -> Result<FireRoutineResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&routine_id)
        .map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

    let routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;

    let thread_id = db
        .get_or_create_routine_conversation(uuid, &routine.name, &state.owner_id)
        .await
        .map_err(|e| format!("Failed to get routine conversation: {}", e))?;

    let prompt = match &routine.action {
        RoutineAction::Lightweight { prompt, .. } => prompt.clone(),
        RoutineAction::FullJob { description, .. } => description.clone(),
    };

    tracing::info!(routine_id = %routine_id, thread_id = %thread_id, "Routine prepared for chat flow");
    Ok(FireRoutineResponse {
        thread_id: thread_id.to_string(),
        prompt,
    })
}
