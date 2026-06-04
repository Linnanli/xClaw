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

fn routine_run_to_response(run: &ironclaw::agent::routine::RoutineRun) -> RoutineRun {
    RoutineRun {
        id: run.id.to_string(),
        trigger_type: run.trigger_type.clone(),
        started_at: run.started_at.to_rfc3339(),
        completed_at: run.completed_at.map(|dt| dt.to_rfc3339()),
        status: format!("{:?}", run.status),
        result_summary: run.result_summary.clone(),
        tokens_used: run.tokens_used,
        job_id: run.job_id.map(|id| id.to_string()),
    }
}

fn ensure_owned_routine(state: &crate::state::AppState, routine: &Routine) -> Result<(), String> {
    if routine.user_id == state.scope_id {
        return Ok(());
    }

    Err("Routine not found or access denied".into())
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

pub(crate) fn normalize_cron_timezone(trigger: Trigger) -> Trigger {
    match trigger {
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
    }
}

pub(crate) fn routine_next_fire_at(
    trigger: &Trigger,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    match trigger {
        Trigger::Cron { schedule, timezone } => next_cron_fire(schedule, timezone.as_deref())
            .map_err(|e| format!("Invalid cron expression: {}", e)),
        _ => Ok(None),
    }
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
    list_routines_from_state(state).await
}

/// 若 RoutineEngine 已就绪，刷新内存事件缓存。
async fn refresh_event_cache(state: &crate::state::AppState) {
    if let Some(engine) = state.routine_engine_slot.read().await.as_ref() {
        engine.refresh_event_cache().await;
    }
}

async fn list_routines_from_state(
    state: &crate::state::AppState,
) -> Result<Vec<RoutineInfo>, String> {
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

async fn create_routine_from_state(
    state: &crate::state::AppState,
    request: CreateRoutineRequest,
) -> Result<RoutineInfo, String> {
    let db = state.db.as_ref().ok_or("Database not available")?;

    let trigger: Trigger = serde_json::from_value(request.trigger)
        .map_err(|e| format!("Invalid trigger format: {}", e))?;

    let trigger = normalize_cron_timezone(trigger);
    let next_fire_at = routine_next_fire_at(&trigger)?;

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

async fn toggle_routine_from_state(
    state: &crate::state::AppState,
    routine_id: String,
    enabled: bool,
) -> Result<(), String> {
    let db = state.db.as_ref().ok_or("Database not available")?;
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let mut routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;
    ensure_owned_routine(state, &routine)?;

    routine.enabled = enabled;
    routine.updated_at = chrono::Utc::now();

    db.update_routine(&routine)
        .await
        .map_err(|e| format!("Failed to update routine: {}", e))?;

    refresh_event_cache(state).await;

    tracing::debug!(id = %routine_id, enabled, "Routine toggled");
    Ok(())
}

async fn delete_routine_from_state(
    state: &crate::state::AppState,
    routine_id: String,
) -> Result<(), String> {
    let db = state.db.as_ref().ok_or("Database not available")?;
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;
    ensure_owned_routine(state, &routine)?;

    db.delete_routine(uuid)
        .await
        .map_err(|e| format!("Failed to delete routine: {}", e))?;

    refresh_event_cache(state).await;

    tracing::info!(id = %routine_id, "Routine deleted");
    Ok(())
}

async fn routine_runs_from_state(
    state: &crate::state::AppState,
    routine_id: String,
) -> Result<RoutineRunsResponse, String> {
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;
    let routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;
    ensure_owned_routine(state, &routine)?;

    let runs = db
        .list_routine_runs(uuid, 50)
        .await
        .map_err(|e| format!("Failed to load routine runs: {}", e))?;

    Ok(RoutineRunsResponse {
        routine_id,
        runs: runs.iter().map(routine_run_to_response).collect(),
    })
}

async fn fire_routine_from_state(
    state: &crate::state::AppState,
    routine_id: String,
) -> Result<TriggerRoutineResponse, String> {
    let uuid =
        Uuid::parse_str(&routine_id).map_err(|_| format!("Invalid routine ID: {}", routine_id))?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let routine = db
        .get_routine(uuid)
        .await
        .map_err(|e| format!("Failed to get routine: {}", e))?
        .ok_or_else(|| format!("Routine not found: {}", routine_id))?;
    ensure_owned_routine(state, &routine)?;

    let mut routine = routine;
    if maybe_upgrade_legacy_lightweight(&mut routine) {
        db.update_routine(&routine)
            .await
            .map_err(|e| format!("Failed to upgrade routine action: {}", e))?;
        refresh_event_cache(state).await;
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

/// 创建新日程。
#[tauri::command]
pub async fn ic_create_routine(
    state: State<'_, EngineState>,
    request: CreateRoutineRequest,
) -> Result<RoutineInfo, String> {
    let state = state.get()?;
    create_routine_from_state(state, request).await
}

/// 启用或禁用日程。
#[tauri::command]
pub async fn ic_toggle_routine(
    state: State<'_, EngineState>,
    routine_id: String,
    enabled: bool,
) -> Result<(), String> {
    let state = state.get()?;
    toggle_routine_from_state(state, routine_id, enabled).await
}

/// 删除日程。
#[tauri::command]
pub async fn ic_delete_routine(
    state: State<'_, EngineState>,
    routine_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    delete_routine_from_state(state, routine_id).await
}

/// 获取日程执行历史。
#[tauri::command]
pub async fn ic_routine_runs(
    state: State<'_, EngineState>,
    routine_id: String,
) -> Result<RoutineRunsResponse, String> {
    let state = state.get()?;
    routine_runs_from_state(state, routine_id).await
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
    fire_routine_from_state(state, routine_id).await
}

#[cfg(test)]
mod tests {
    use super::{
        create_routine_from_state, delete_routine_from_state, fire_routine_from_state,
        list_routines_from_state, maybe_upgrade_legacy_lightweight, routine_runs_from_state,
        toggle_routine_from_state,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::AppState;
    use ironclaw::agent::routine::RoutineRun as DbRoutineRun;
    use ironclaw::agent::routine::{
        NotifyConfig, Routine, RoutineAction, RoutineGuardrails, RunStatus, Trigger,
    };
    use ironclaw::db::libsql::LibSqlBackend;
    use ironclaw::db::Database;
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;
    use std::collections::HashSet;
    use std::sync::Arc;
    use uuid::Uuid;

    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }

        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }

        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    struct RoutinesStateFixture {
        state: AppState,
        db: Arc<dyn Database>,
        _tempdir: tempfile::TempDir,
    }

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

    async fn create_routines_state_fixture() -> RoutinesStateFixture {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("routines.db");
        let backend = LibSqlBackend::new_local(&db_path)
            .await
            .expect("create libsql backend");
        backend.run_migrations().await.expect("run migrations");
        let db: Arc<dyn Database> = Arc::new(backend);

        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));
        let safety_bridge = Arc::new(SafetyBridge::new(Arc::clone(&safety), None, None));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );
        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        let state = AppState {
            msg_sender: tx,
            db: Some(Arc::clone(&db)),
            workspace: None,
            tools: Arc::new(ToolRegistry::new()),
            extension_manager: None,
            skill_registry: None,
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
            context_manager: Arc::new(dasclaw_runtime::context::ContextManager::new(5)),
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "test-owner".to_string(),
            )),
            data_reporter: Arc::new(crate::data_reporter::DataReporter::new_for_test()),
            scope_id: "test-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: Arc::clone(&stub_llm),
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(HashSet::new()),
        };

        RoutinesStateFixture {
            state,
            db,
            _tempdir: tempdir,
        }
    }

    async fn save_test_routine(db: &Arc<dyn Database>, user_id: &str, name: &str) -> Routine {
        let mut routine = sample_routine(RoutineAction::FullJob {
            title: name.to_string(),
            description: "Generate summary".to_string(),
            max_iterations: 25,
        });
        routine.user_id = user_id.to_string();
        routine.name = name.to_string();
        routine.description = "Daily summary".to_string();
        db.create_routine(&routine).await.expect("create routine");
        routine
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

    #[tokio::test]
    async fn req_routines_state_round_trip_create_list_toggle_runs_and_delete() {
        let fixture = create_routines_state_fixture().await;

        let created = create_routine_from_state(
            &fixture.state,
            super::CreateRoutineRequest {
                name: "Daily report".to_string(),
                description: "Every morning".to_string(),
                trigger: serde_json::json!({"type": "manual"}),
                prompt: "Generate daily report".to_string(),
            },
        )
        .await
        .expect("create routine");
        assert_eq!(created.name, "Daily report");
        assert_eq!(created.status, "active");

        save_test_routine(&fixture.db, "other-owner", "Hidden routine").await;

        let routines = list_routines_from_state(&fixture.state)
            .await
            .expect("list routines");
        assert_eq!(routines.len(), 1);
        assert_eq!(routines[0].id, created.id);

        toggle_routine_from_state(&fixture.state, created.id.clone(), false)
            .await
            .expect("toggle routine");
        let routine_id = Uuid::parse_str(&created.id).expect("created id should parse");
        let toggled = fixture
            .db
            .get_routine(routine_id)
            .await
            .expect("load toggled routine")
            .expect("routine should exist");
        assert!(!toggled.enabled);

        let run_id = Uuid::new_v4();
        fixture
            .db
            .create_routine_run(&DbRoutineRun {
                id: run_id,
                routine_id,
                trigger_type: "manual".to_string(),
                trigger_detail: Some("button".to_string()),
                started_at: chrono::Utc::now(),
                completed_at: None,
                status: RunStatus::Running,
                result_summary: None,
                tokens_used: None,
                job_id: None,
                created_at: chrono::Utc::now(),
            })
            .await
            .expect("create routine run");
        fixture
            .db
            .complete_routine_run(run_id, RunStatus::Ok, Some("done"), Some(42))
            .await
            .expect("complete routine run");

        let runs = routine_runs_from_state(&fixture.state, created.id.clone())
            .await
            .expect("routine runs");
        assert_eq!(runs.routine_id, created.id);
        assert_eq!(runs.runs.len(), 1);
        assert_eq!(runs.runs[0].id, run_id.to_string());
        assert_eq!(runs.runs[0].status, "Ok");
        assert_eq!(runs.runs[0].result_summary.as_deref(), Some("done"));
        assert_eq!(runs.runs[0].tokens_used, Some(42));

        delete_routine_from_state(&fixture.state, created.id.clone())
            .await
            .expect("delete routine");
        let deleted = fixture
            .db
            .get_routine(routine_id)
            .await
            .expect("load deleted routine");
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn req_routines_state_rejects_unowned_toggle_runs_delete_and_fire() {
        let fixture = create_routines_state_fixture().await;
        let routine = save_test_routine(&fixture.db, "other-owner", "Hidden routine").await;
        let routine_id = routine.id.to_string();

        let toggle_error = toggle_routine_from_state(&fixture.state, routine_id.clone(), false)
            .await
            .expect_err("toggle should enforce owner scope");
        assert_eq!(toggle_error, "Routine not found or access denied");

        let runs_error = routine_runs_from_state(&fixture.state, routine_id.clone())
            .await
            .expect_err("runs should enforce owner scope");
        assert_eq!(runs_error, "Routine not found or access denied");

        let delete_error = delete_routine_from_state(&fixture.state, routine_id.clone())
            .await
            .expect_err("delete should enforce owner scope");
        assert_eq!(delete_error, "Routine not found or access denied");

        let fire_error = fire_routine_from_state(&fixture.state, routine_id)
            .await
            .expect_err("fire should enforce owner scope");
        assert_eq!(fire_error, "Routine not found or access denied");
    }

    #[tokio::test]
    async fn req_routines_fire_upgrades_legacy_lightweight_before_engine_required() {
        let fixture = create_routines_state_fixture().await;
        let mut routine = sample_routine(RoutineAction::Lightweight {
            prompt: "Legacy prompt".to_string(),
            context_paths: vec![],
            max_tokens: 1024,
            use_tools: false,
            max_tool_rounds: 1,
        });
        routine.user_id = "test-owner".to_string();
        fixture
            .db
            .create_routine(&routine)
            .await
            .expect("create legacy routine");

        let error = fire_routine_from_state(&fixture.state, routine.id.to_string())
            .await
            .expect_err("missing engine should be reported after upgrade");
        assert_eq!(error, "Routine engine not available");

        let upgraded = fixture
            .db
            .get_routine(routine.id)
            .await
            .expect("load upgraded routine")
            .expect("routine should exist");
        match upgraded.action {
            RoutineAction::FullJob {
                title,
                description,
                max_iterations,
            } => {
                assert_eq!(title, "测试任务");
                assert_eq!(description, "Legacy prompt");
                assert_eq!(max_iterations, 25);
            }
            _ => panic!("expected upgraded full_job action"),
        }
    }
}
