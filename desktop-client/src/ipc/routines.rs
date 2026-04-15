//! 日程管理 Tauri Commands。
//!
//! 直接调用 IronClaw 的 `Database` 组件（`RoutineStore` trait）管理定时任务。

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::state::EngineState;
use ironclaw::agent::routine::next_cron_fire;
use ironclaw::agent::routine::{Routine, RoutineAction, RoutineGuardrails, Trigger};

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

/// 将历史的 lightweight routine 统一升级为 full_job，确保触发后进入任务系统。
fn maybe_upgrade_legacy_lightweight(routine: &mut Routine) -> bool {
    let prompt = match &routine.action {
        RoutineAction::Lightweight { prompt, .. } => prompt.clone(),
        RoutineAction::FullJob { .. } => return false,
    };

    let description = if prompt.trim().is_empty() {
        routine.description.clone()
    } else {
        prompt
    };

    routine.action = RoutineAction::FullJob {
        title: routine.name.clone(),
        description,
        max_iterations: 25,
    };
    routine.updated_at = chrono::Utc::now();
    true
}

/// 等待 routine run 被链接到 job（异步调度存在短暂落库延迟）。
async fn wait_for_run_job_link(
    db: &std::sync::Arc<dyn ironclaw::db::Database>,
    routine_id: Uuid,
    run_id: Uuid,
) {
    for _ in 0..20 {
        if let Ok(runs) = db.list_routine_runs(routine_id, 20).await {
            let linked = runs
                .iter()
                .find(|run| run.id == run_id)
                .and_then(|run| run.job_id)
                .is_some();
            if linked {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 列出所有日程。
#[tauri::command]
pub async fn ic_list_routines(state: State<'_, EngineState>) -> Result<Vec<RoutineInfo>, String> {
    let state = state.get()?;
    let db = state.db.as_ref().ok_or("Database not available")?;
    let mut routines = db
        .list_routines(&state.scope_id)
        .await
        .map_err(|e| format!("Failed to list routines: {}", e))?;

    let mut upgraded_any = false;
    for routine in routines.iter_mut() {
        if !maybe_upgrade_legacy_lightweight(routine) {
            continue;
        }
        match db.update_routine(routine).await {
            Ok(_) => upgraded_any = true,
            Err(e) => tracing::warn!(
                routine_id = %routine.id,
                error = %e,
                "Failed to upgrade legacy lightweight routine to full_job"
            ),
        }
    }

    if upgraded_any {
        refresh_event_cache(state).await;
    }

    Ok(routines.iter().map(routine_to_info).collect())
}

/// 若 RoutineEngine 已就绪，刷新内存事件缓存。
async fn refresh_event_cache(state: &crate::state::AppState) {
    if let Some(engine) = state.routine_engine_slot.read().await.as_ref() {
        engine.refresh_event_cache().await;
    }
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
        Trigger::Cron {
            schedule,
            timezone: None,
        } => {
            let local_tz = ironclaw::timezone::detect_system_timezone().to_string();
            Trigger::Cron {
                schedule,
                timezone: Some(local_tz),
            }
        }
        other => other,
    };

    // 对 cron 触发器计算初始 next_fire_at，否则引擎永远不会调度该任务
    let next_fire_at = match &trigger {
        Trigger::Cron { schedule, timezone } => next_cron_fire(schedule, timezone.as_deref())
            .map_err(|e| format!("Invalid cron expression: {}", e))?,
        _ => None,
    };

    let now = chrono::Utc::now();
    let routine = Routine {
        id: Uuid::new_v4(),
        name: request.name.clone(),
        description: request.description,
        user_id: state.scope_id.clone(),
        enabled: true,
        trigger,
        action: RoutineAction::FullJob {
            title: request.name,
            description: request.prompt,
            max_iterations: 25,
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

    refresh_event_cache(state).await;

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
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

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

    refresh_event_cache(state).await;

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
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    db.delete_routine(uuid)
        .await
        .map_err(|e| format!("Failed to delete routine: {}", e))?;

    refresh_event_cache(state).await;

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
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

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

    Ok(RoutineRunsResponse {
        routine_id,
        runs: items,
    })
}

/// 手动触发日程立即执行的响应。
#[derive(Debug, Serialize)]
pub struct TriggerRoutineResponse {
    pub status: String,
    pub routine_id: String,
    pub run_id: String,
}

/// 手动触发日程立即执行。
///
/// 触发 RoutineEngine 执行（会创建 routine run，full_job 会创建 job）。
#[tauri::command]
pub async fn ic_fire_routine(
    state: State<'_, crate::state::EngineState>,
    routine_id: String,
) -> Result<TriggerRoutineResponse, String> {
    let state = state.get()?;
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    // 兼容历史数据：如果是 lightweight，先升级为 full_job，再触发执行。
    let routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?;
    if let Some(mut routine) = routine {
        if maybe_upgrade_legacy_lightweight(&mut routine) {
            db.update_routine(&routine)
                .await
                .map_err(|e| format!("Failed to upgrade routine action: {}", e))?;
            refresh_event_cache(state).await;
        }
    }

    let engine = {
        let guard = state.routine_engine_slot.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or("Routine engine not available".to_string())?
    };

    let run_id = engine
        .fire_manual(uuid, Some(&state.scope_id))
        .await
        .map_err(|e| e.to_string())?;
    wait_for_run_job_link(db, uuid, run_id).await;

    tracing::info!(
        routine_id = %routine_id,
        run_id = %run_id,
        "Routine triggered via routine engine"
    );
    Ok(TriggerRoutineResponse {
        status: "triggered".to_string(),
        routine_id: uuid.to_string(),
        run_id: run_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::maybe_upgrade_legacy_lightweight;
    use ironclaw::agent::routine::{
        NotifyConfig, Routine, RoutineAction, RoutineGuardrails, Trigger,
    };
    use uuid::Uuid;

    fn sample_routine(action: RoutineAction) -> Routine {
        let now = chrono::Utc::now();
        Routine {
            id: Uuid::new_v4(),
            name: "测试任务".to_string(),
            description: "描述".to_string(),
            user_id: "u1".to_string(),
            enabled: true,
            trigger: Trigger::Manual,
            action,
            guardrails: RoutineGuardrails::default(),
            notify: NotifyConfig::default(),
            last_run_at: None,
            next_fire_at: None,
            run_count: 0,
            consecutive_failures: 0,
            state: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_upgrade_legacy_lightweight_to_full_job() {
        let mut routine = sample_routine(RoutineAction::Lightweight {
            prompt: "请生成模型报告".to_string(),
            context_paths: vec![],
            max_tokens: 4096,
            use_tools: false,
            max_tool_rounds: 3,
        });
        assert!(maybe_upgrade_legacy_lightweight(&mut routine));
        match routine.action {
            RoutineAction::FullJob {
                title,
                description,
                max_iterations,
            } => {
                assert_eq!(title, "测试任务");
                assert_eq!(description, "请生成模型报告");
                assert_eq!(max_iterations, 25);
            }
            _ => panic!("expected full_job action"),
        }
    }

    #[test]
    fn test_keep_full_job_unchanged() {
        let mut routine = sample_routine(RoutineAction::FullJob {
            title: "测试任务".to_string(),
            description: "请生成模型报告".to_string(),
            max_iterations: 15,
        });
        assert!(!maybe_upgrade_legacy_lightweight(&mut routine));
        match routine.action {
            RoutineAction::FullJob { max_iterations, .. } => assert_eq!(max_iterations, 15),
            _ => panic!("expected full_job action"),
        }
    }
}
