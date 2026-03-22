//! 日程管理扩展 Tauri Commands。
//!
//! 提供日程执行历史查询功能，直接调用 IronClaw 的 `Database` 组件。

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

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

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 获取日程执行历史。
///
/// 从数据库加载最近 50 条执行记录。
#[tauri::command]
pub async fn ic_routine_runs(
    state: State<'_, EngineState>,
    routine_id: String,
) -> Result<RoutineRunsResponse, String> {
    let state = state.get()?;
    let uuid = Uuid::parse_str(&routine_id)
        .map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let db = state
        .db
        .as_ref()
        .ok_or("Database not available")?;

    let runs = db
        .list_routine_runs(uuid, 50)
        .await
        .map_err(|e| format!("Failed to load routine runs: {}", e))?;

    let items: Vec<RoutineRun> = runs
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

    tracing::debug!(
        routine_id = %routine_id,
        count = items.len(),
        "Routine runs loaded"
    );

    Ok(RoutineRunsResponse {
        routine_id,
        runs: items,
    })
}
