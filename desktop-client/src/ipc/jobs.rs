//! 任务管理 Tauri Commands。
//!
//! 直接调用 IronClaw 的 `Database` 和 `Scheduler` 组件，
//! 提供任务事件历史和后续提示功能。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::state::EngineState;
use crate::vercel_ui_protocol::VercelUIStream;
use dasclaw_runtime::{context::JobContext, JobState};
use ironclaw::channels::IncomingMessage;
use ironclaw::history::{AgentJobRecord, JobEventRecord};

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
    job: &dasclaw_runtime::context::JobContext,
) -> Result<(), String> {
    if job.user_id == state.scope_id {
        return Ok(());
    }

    Err("Job not found or access denied".into())
}

async fn stop_active_job(
    scheduler: Option<std::sync::Arc<ironclaw::agent::JobDispatcher>>,
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
    let event = VercelUIStream::DataCustom {
        id: None,
        data: serde_json::json!({
            "type": "job_status",
            "job_id": job_id.to_string(),
            "title": title,
            "status": status,
        }),
    };
    // 全局 job 事件：未绑定到特定 thread，前端所有 Transport 透传
    let _ = crate::tauri_channel::emit_chat_stream(app_handle, None, &event);
}

fn restart_job_title(job: &JobContext, failure_reason: &str) -> String {
    if failure_reason.is_empty() || matches!(job.state, JobState::Cancelled) {
        return job.title.clone();
    }

    format!(
        "Previous attempt failed: {}. Retry: {}",
        failure_reason, job.title
    )
}

fn job_record_to_info(job: AgentJobRecord, conversation_id: Option<Uuid>) -> JobInfoResponse {
    JobInfoResponse {
        id: job.id.to_string(),
        title: job.title,
        status: job.status,
        created_at: job.created_at.to_rfc3339(),
        started_at: job.started_at.map(|time| time.to_rfc3339()),
        completed_at: job.completed_at.map(|time| time.to_rfc3339()),
        conversation_id: conversation_id.map(|id| id.to_string()),
    }
}

fn job_event_to_response(event: JobEventRecord) -> JobEvent {
    JobEvent {
        id: event.id,
        event_type: event.event_type,
        data: event.data,
        created_at: event.created_at.to_rfc3339(),
    }
}

fn job_events_response(job_id: String, events: Vec<JobEventRecord>) -> JobEventsResponse {
    JobEventsResponse {
        job_id,
        events: events.into_iter().map(job_event_to_response).collect(),
    }
}

fn job_detail_to_response(
    job_id: String,
    ctx: JobContext,
    failure_reason: Option<String>,
    events: Vec<JobEventRecord>,
) -> JobDetailResponse {
    JobDetailResponse {
        id: job_id,
        title: ctx.title,
        description: ctx.description,
        status: ctx.state.to_string(),
        source: "direct".to_string(),
        created_at: ctx.created_at.to_rfc3339(),
        started_at: ctx.started_at.map(|time| time.to_rfc3339()),
        completed_at: ctx.completed_at.map(|time| time.to_rfc3339()),
        conversation_id: ctx.conversation_id.map(|id| id.to_string()),
        failure_reason,
        total_tokens_used: (ctx.total_tokens_used > 0).then_some(ctx.total_tokens_used),
        events: events.into_iter().map(job_event_to_response).collect(),
    }
}

fn build_job_prompt_message(scope_id: &str, job_id: Uuid, content: &str) -> IncomingMessage {
    let prompt_content = format!("!prompt {} {}", job_id, content);
    IncomingMessage::new("tauri", scope_id, prompt_content).with_owner_id(scope_id)
}

fn job_prompt_response(job_id: String) -> JobPromptResponse {
    JobPromptResponse {
        status: "sent".to_string(),
        job_id,
    }
}

async fn list_jobs_from_state(
    state: &crate::state::AppState,
) -> Result<Vec<JobInfoResponse>, String> {
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
                .and_then(|ctx| ctx.conversation_id);
            (idx, job_record_to_info(j, conversation_id))
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

async fn get_job_detail_from_state(
    state: &crate::state::AppState,
    job_id: String,
) -> Result<JobDetailResponse, String> {
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

    let ctx = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;

    ensure_owned_job(state, &ctx)?;

    let failure_reason = if ctx.state == JobState::Failed {
        db.get_agent_job_failure_reason(uuid).await.unwrap_or(None)
    } else {
        None
    };

    let events = db.list_job_events(uuid, None).await.unwrap_or_default();

    Ok(job_detail_to_response(job_id, ctx, failure_reason, events))
}

async fn job_events_from_state(
    state: &crate::state::AppState,
    job_id: String,
) -> Result<JobEventsResponse, String> {
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;

    let job = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;
    ensure_owned_job(state, &job)?;

    let events = db
        .list_job_events(uuid, None)
        .await
        .map_err(|e| format!("Failed to load job events: {}", e))?;

    let response = job_events_response(job_id, events);

    tracing::debug!(job_id = %response.job_id, count = response.events.len(), "Job events loaded");

    Ok(response)
}

async fn job_prompt_from_state(
    state: &crate::state::AppState,
    job_id: String,
    content: String,
) -> Result<JobPromptResponse, String> {
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;

    let db = state.db.as_ref().ok_or("Database not available")?;
    let job = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;
    ensure_owned_job(state, &job)?;

    let msg = build_job_prompt_message(&state.scope_id, uuid, &content);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send prompt: {}", e))?;

    tracing::info!(job_id = %job_id, "Follow-up prompt sent");

    Ok(job_prompt_response(job_id))
}

async fn cancel_job_from_state(
    state: &crate::state::AppState,
    job_id: String,
) -> Result<(Uuid, String), String> {
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

    tracing::info!(job_id = %uuid, "Job cancelled");
    Ok((uuid, job.title))
}

async fn restart_job_from_state(
    state: &crate::state::AppState,
    job_id: String,
) -> Result<(Uuid, String), String> {
    let uuid = Uuid::parse_str(&job_id).map_err(|_| format!("Invalid job ID: {}", job_id))?;
    let db = state.db.as_ref().ok_or("Database not available")?;

    let job = db
        .get_job(uuid)
        .await
        .map_err(|e| format!("Failed to load job: {}", e))?
        .ok_or_else(|| format!("Job not found: {}", job_id))?;

    ensure_owned_job(state, &job)?;

    if job.state.is_active() {
        return Err(format!(
            "Cannot restart active job in state '{}'",
            job.state
        ));
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

    tracing::info!(old_job_id = %uuid, new_job_id = %new_job_id, "Job restarted");
    Ok((new_job_id, title))
}

// ─── Tauri Commands ──────────────────────────────────────────────────

/// 列出当前用户的所有任务。
#[tauri::command]
pub async fn ic_list_jobs(state: State<'_, EngineState>) -> Result<Vec<JobInfoResponse>, String> {
    let state = state.get()?;
    list_jobs_from_state(state).await
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
    get_job_detail_from_state(state, job_id).await
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
    job_events_from_state(state, job_id).await
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
    job_prompt_from_state(state, job_id, content).await
}

/// 取消运行中的 agent 任务。
#[tauri::command]
pub async fn ic_cancel_job(
    app_handle: tauri::AppHandle,
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let (uuid, title) = cancel_job_from_state(state, job_id).await?;
    emit_job_status(&app_handle, uuid, &title, "cancelled");
    Ok(())
}

/// 重试一个已结束的 agent 任务。
#[tauri::command]
pub async fn ic_restart_job(
    app_handle: tauri::AppHandle,
    state: State<'_, EngineState>,
    job_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    let (new_job_id, title) = restart_job_from_state(state, job_id).await?;
    emit_job_status(&app_handle, new_job_id, &title, "in_progress");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_job_prompt_message, cancel_job_from_state, get_job_detail_from_state,
        job_detail_to_response, job_event_to_response, job_events_from_state, job_events_response,
        job_prompt_from_state, job_prompt_response, job_record_to_info, list_jobs_from_state,
        restart_job_from_state, restart_job_title, stop_active_job,
    };
    use crate::safety_bridge::SafetyBridge;
    use crate::state::AppState;
    use chrono::{TimeZone, Utc};
    use dasclaw_runtime::{context::JobContext, JobState};
    use ironclaw::channels::IncomingMessage;
    use ironclaw::db::libsql::LibSqlBackend;
    use ironclaw::db::Database;
    use ironclaw::history::{AgentJobRecord, JobEventRecord};
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use ironclaw::tools::ToolRegistry;
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::sync::mpsc;
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

    struct JobsStateFixture {
        state: AppState,
        db: Arc<dyn Database>,
        rx: mpsc::Receiver<IncomingMessage>,
        _tempdir: tempfile::TempDir,
    }

    fn utc_time(second: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 3, 10, 20, second)
            .single()
            .expect("test timestamp should be valid")
    }

    fn sample_event(id: i64, job_id: Uuid, event_type: &str) -> JobEventRecord {
        JobEventRecord {
            id,
            job_id,
            event_type: event_type.to_string(),
            data: serde_json::json!({"step": id}),
            created_at: utc_time(id as u32),
        }
    }

    async fn create_jobs_state_fixture() -> JobsStateFixture {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("jobs.db");
        let backend = LibSqlBackend::new_local(&db_path)
            .await
            .expect("create libsql backend");
        backend.run_migrations().await.expect("run migrations");
        let db: Arc<dyn Database> = Arc::new(backend);

        let (tx, rx) = mpsc::channel(8);
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

        JobsStateFixture {
            state,
            db,
            rx,
            _tempdir: tempdir,
        }
    }

    async fn save_test_job(
        db: &Arc<dyn Database>,
        user_id: &str,
        title: &str,
        job_state: JobState,
    ) -> JobContext {
        let mut job = JobContext::with_user(user_id, title, "Build summary");
        job.state = job_state;
        job.created_at = utc_time(10);
        job.started_at = Some(utc_time(11));
        job.completed_at = Some(utc_time(12));
        job.conversation_id = Some(
            db.create_conversation("tauri", user_id, None)
                .await
                .expect("create conversation"),
        );
        db.save_job(&job).await.expect("save job");
        job
    }

    #[test]
    fn req_jobs_list_maps_record_without_internal_owner_fields() {
        let job_id = Uuid::nil();
        let conversation_id = Uuid::new_v4();
        let record = AgentJobRecord {
            id: job_id,
            title: "Nightly report".to_string(),
            status: "in_progress".to_string(),
            user_id: "owner-1".to_string(),
            created_at: utc_time(1),
            started_at: Some(utc_time(2)),
            completed_at: None,
            failure_reason: Some("hidden".to_string()),
        };

        let response = job_record_to_info(record, Some(conversation_id));
        let json = serde_json::to_string(&response).expect("should serialize");

        assert_eq!(response.id, job_id.to_string());
        assert_eq!(response.title, "Nightly report");
        assert_eq!(response.status, "in_progress");
        assert_eq!(response.conversation_id, Some(conversation_id.to_string()));
        assert!(!json.contains("owner-1"));
        assert!(!json.contains("hidden"));
    }

    #[test]
    fn req_jobs_events_maps_records_in_order() {
        let job_id = Uuid::new_v4();
        let response = job_events_response(
            job_id.to_string(),
            vec![
                sample_event(1, job_id, "started"),
                sample_event(2, job_id, "completed"),
            ],
        );

        assert_eq!(response.job_id, job_id.to_string());
        assert_eq!(response.events.len(), 2);
        assert_eq!(response.events[0].event_type, "started");
        assert_eq!(response.events[1].data, serde_json::json!({"step": 2}));
    }

    #[test]
    fn req_jobs_detail_maps_context_failure_and_events() {
        let job_id = Uuid::new_v4();
        let conversation_id = Uuid::new_v4();
        let mut context = JobContext::with_user("owner-1", "Report", "Build summary");
        context.job_id = job_id;
        context.state = JobState::Failed;
        context.conversation_id = Some(conversation_id);
        context.created_at = utc_time(3);
        context.started_at = Some(utc_time(4));
        context.completed_at = Some(utc_time(5));
        context.total_tokens_used = 42;

        let response = job_detail_to_response(
            job_id.to_string(),
            context,
            Some("tool failed".to_string()),
            vec![sample_event(6, job_id, "error")],
        );

        assert_eq!(response.id, job_id.to_string());
        assert_eq!(response.title, "Report");
        assert_eq!(response.description, "Build summary");
        assert_eq!(response.status, "failed");
        assert_eq!(response.source, "direct");
        assert_eq!(response.conversation_id, Some(conversation_id.to_string()));
        assert_eq!(response.failure_reason.as_deref(), Some("tool failed"));
        assert_eq!(response.total_tokens_used, Some(42));
        assert_eq!(response.events[0].event_type, "error");
    }

    #[test]
    fn req_jobs_prompt_message_targets_scope_owner() {
        let job_id = Uuid::nil();
        let message = build_job_prompt_message("owner-1", job_id, "continue");

        assert_eq!(message.channel, "tauri");
        assert_eq!(message.user_id, "owner-1");
        assert_eq!(message.owner_id, "owner-1");
        assert_eq!(message.sender_id, "owner-1");
        assert_eq!(message.content, format!("!prompt {} continue", job_id));
        assert!(message.thread_id.is_none());
    }

    #[test]
    fn req_jobs_prompt_response_reports_sent_status() {
        let response = job_prompt_response("job-1".to_string());

        assert_eq!(response.status, "sent");
        assert_eq!(response.job_id, "job-1");
    }

    #[test]
    fn req_jobs_restart_title_prefixes_failed_attempt_reason() {
        let mut context = JobContext::with_user("owner-1", "Nightly report", "Build summary");
        context.state = JobState::Failed;

        let title = restart_job_title(&context, "network timeout");

        assert_eq!(
            title,
            "Previous attempt failed: network timeout. Retry: Nightly report"
        );
    }

    #[test]
    fn req_jobs_restart_title_keeps_cancelled_title() {
        let mut context = JobContext::with_user("owner-1", "Nightly report", "Build summary");
        context.state = JobState::Cancelled;

        let title = restart_job_title(&context, "user cancelled");

        assert_eq!(title, "Nightly report");
    }

    #[tokio::test]
    async fn active_job_without_scheduler_is_rejected() {
        let error = stop_active_job(None, Uuid::nil(), JobState::InProgress)
            .await
            .expect_err("active jobs require scheduler stop before marking cancelled");

        assert_eq!(
            error,
            "Failed to cancel job: active job scheduler unavailable"
        );
    }

    #[tokio::test]
    async fn inactive_job_without_scheduler_is_allowed() {
        stop_active_job(None, Uuid::nil(), JobState::Failed)
            .await
            .expect("inactive job should not require scheduler stop");
    }

    #[tokio::test]
    async fn req_jobs_state_round_trip_lists_details_events_prompts_and_cancels() {
        let mut fixture = create_jobs_state_fixture().await;
        let job = save_test_job(
            &fixture.db,
            "test-owner",
            "Nightly report",
            JobState::Failed,
        )
        .await;
        fixture
            .db
            .update_job_status(job.job_id, JobState::Failed, Some("tool failed"))
            .await
            .expect("persist failure reason");
        fixture
            .db
            .save_job_event(
                job.job_id,
                "failed",
                &serde_json::json!({"reason": "tool failed"}),
            )
            .await
            .expect("save event");
        save_test_job(&fixture.db, "other-owner", "Hidden job", JobState::Failed).await;

        let jobs = list_jobs_from_state(&fixture.state)
            .await
            .expect("list jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, job.job_id.to_string());
        assert_eq!(jobs[0].title, "Nightly report");
        assert_eq!(
            jobs[0].conversation_id,
            job.conversation_id.map(|id| id.to_string())
        );

        let detail = get_job_detail_from_state(&fixture.state, job.job_id.to_string())
            .await
            .expect("job detail");
        assert_eq!(detail.id, job.job_id.to_string());
        assert_eq!(detail.failure_reason.as_deref(), Some("tool failed"));
        assert_eq!(detail.events.len(), 1);
        assert_eq!(detail.events[0].event_type, "failed");

        let events = job_events_from_state(&fixture.state, job.job_id.to_string())
            .await
            .expect("job events");
        assert_eq!(events.job_id, job.job_id.to_string());
        assert_eq!(
            events.events[0].data,
            serde_json::json!({"reason": "tool failed"})
        );

        let prompt = job_prompt_from_state(
            &fixture.state,
            job.job_id.to_string(),
            "continue with summary".to_string(),
        )
        .await
        .expect("job prompt");
        assert_eq!(prompt.status, "sent");
        let message = fixture.rx.recv().await.expect("prompt message");
        assert_eq!(message.owner_id, "test-owner");
        assert_eq!(
            message.content,
            format!("!prompt {} continue with summary", job.job_id)
        );

        let (cancelled_id, cancelled_title) =
            cancel_job_from_state(&fixture.state, job.job_id.to_string())
                .await
                .expect("cancel failed job");
        assert_eq!(cancelled_id, job.job_id);
        assert_eq!(cancelled_title, "Nightly report");
        let cancelled = fixture
            .db
            .get_job(job.job_id)
            .await
            .expect("load cancelled job")
            .expect("job should remain");
        assert_eq!(cancelled.state, JobState::Cancelled);
        let cancel_reason = fixture
            .db
            .get_agent_job_failure_reason(job.job_id)
            .await
            .expect("load cancel reason");
        assert_eq!(cancel_reason.as_deref(), Some("Cancelled by user"));
    }

    #[tokio::test]
    async fn req_jobs_state_rejects_unowned_detail_events_and_prompt() {
        let mut fixture = create_jobs_state_fixture().await;
        let job = save_test_job(&fixture.db, "other-owner", "Hidden job", JobState::Failed).await;
        fixture
            .db
            .save_job_event(job.job_id, "hidden", &serde_json::json!({"secret": true}))
            .await
            .expect("save event");

        let detail_error = get_job_detail_from_state(&fixture.state, job.job_id.to_string())
            .await
            .expect_err("detail should enforce owner scope");
        assert_eq!(detail_error, "Job not found or access denied");

        let events_error = job_events_from_state(&fixture.state, job.job_id.to_string())
            .await
            .expect_err("events should enforce owner scope");
        assert_eq!(events_error, "Job not found or access denied");

        let prompt_error = job_prompt_from_state(
            &fixture.state,
            job.job_id.to_string(),
            "leak please".to_string(),
        )
        .await
        .expect_err("prompt should enforce owner scope");
        assert_eq!(prompt_error, "Job not found or access denied");
        assert!(fixture.rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn req_jobs_restart_rejects_active_job_before_scheduler_dispatch() {
        let fixture = create_jobs_state_fixture().await;
        let job = save_test_job(
            &fixture.db,
            "test-owner",
            "Still running",
            JobState::InProgress,
        )
        .await;

        let error = restart_job_from_state(&fixture.state, job.job_id.to_string())
            .await
            .expect_err("active job should not restart");

        assert_eq!(error, "Cannot restart active job in state 'in_progress'");
    }

    #[test]
    fn req_jobs_event_mapping_preserves_timestamp_and_payload() {
        let job_id = Uuid::new_v4();
        let response = job_event_to_response(sample_event(7, job_id, "thinking"));

        assert_eq!(response.id, 7);
        assert_eq!(response.event_type, "thinking");
        assert_eq!(response.data, serde_json::json!({"step": 7}));
        assert!(response.created_at.starts_with("2026-06-03T10:20:07"));
    }
}
