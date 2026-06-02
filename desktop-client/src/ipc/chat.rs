//! 聊天相关 Tauri Commands。
//!
//! 通过 `AppState.msg_sender` 将用户消息注入 TauriChannel，
//! Agent 处理后通过 `VercelUIStream` 推送回复到前端。
//!
//! # 前端兼容性
//!
//! 命令名与现有前端 `useAiChatTauri.ts` 完全匹配：
//! - `send_chat_message` — 发送消息
//! - `subscribe_chat_events` — 订阅事件（新架构下为 no-op）
//! - `unsubscribe_chat_events` — 取消订阅（新架构下为 no-op）

use std::sync::Arc;

use ironclaw::channels::{AttachmentKind, IncomingAttachment, IncomingMessage};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};
use tracing::Instrument;

use crate::data_reporter::ConversationAttachment;
use crate::safety_attachment_scanner::{AttachmentDecision, AttachmentScanner};
use crate::safety_bridge::BridgeScanResult;
use crate::state::EngineState;
use crate::vercel_ui_protocol::VercelUIStream;

/// 发送消息的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub success: bool,
}

/// 前端可选传入的 DLP 脱敏统计（用于持久化 UI 标记）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDlpStats {
    pub total_matches: usize,
    pub redacted_count: usize,
    pub blocked_count: usize,
    pub warned_count: usize,
}

/// 前端上传到 Tauri 的附件负载。
#[derive(Debug, Clone, Deserialize)]
pub struct FrontendAttachment {
    pub id: String,
    pub kind: String,
    pub mime_type: String,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub extracted_text: Option<String>,
    pub data: Vec<u8>,
    pub duration_secs: Option<u32>,
}

/// 发送聊天消息。
///
/// 构造 `IncomingMessage` 并通过 `msg_sender` 注入 Agent 消息循环。
/// AI 回复通过 `chat-stream` Tauri 事件异步推送到前端。
///
/// # 模型切换
///
/// 双重机制，统一在此函数中处理：
/// 1. `set_model()` — 对支持的 provider 直接切换活跃模型
/// 2. `model_override` — 对不支持 `set_model` 的 provider（如 rig-core），
///    由 `ModelSwitchProvider` 在每次 LLM 调用时注入 `request.model`
///
/// # Skill 激活通知
///
/// 在消息注入 Agent 前，先用 ironclaw 的 `prefilter_skills` 做本地匹配。
/// 若有 skill 被激活，通过 `chat-stream` 发出 `skills_activated` 事件。
///
/// # 安全
///
/// - 消息内容先经过 SafetyBridge 扫描（密钥检测 + PII 脱敏）
/// - 检测到密钥时**故障安全**拒绝发送
/// - PII 信息格式保留脱敏后再发送给 Agent
/// - 消息内容不写入日志（防止敏感信息泄露）
/// - 仅记录 message_id 和 thread_id 用于追踪
#[tauri::command]
pub async fn send_chat_message(
    app_handle: tauri::AppHandle,
    state: State<'_, EngineState>,
    thread_id: String,
    content: String,
    model_id: Option<String>,
    api_base_url: Option<String>,
    api_key: Option<String>,
    dlp_stats: Option<ClientDlpStats>,
    attachments: Option<Vec<FrontendAttachment>>,
) -> Result<SendMessageResponse, String> {
    let state = state.get()?;
    let message_id = uuid::Uuid::new_v4().to_string();

    // Instrumentation for issue #1016: per-step latency under target
    // `ironclaw::startup_latency`. Filter with
    // `RUST_LOG=ironclaw::startup_latency=info`.
    let attach_span = tracing::info_span!(
        target: "ironclaw::startup_latency",
        "send_chat_message.build_attachments",
        message_id = %message_id,
    );
    let (incoming_attachments, report_attachments) =
        build_attachments(attachments, state.attachment_scanner.as_ref())
            .instrument(attach_span)
            .await?;

    // ── 配额预检：调用 Admin Backend 检查是否超额 ─────────────────
    let quota_span = tracing::info_span!(
        target: "ironclaw::startup_latency",
        "send_chat_message.quota_precheck",
        message_id = %message_id,
    );
    if let Err(reason) = quota_precheck(state).instrument(quota_span).await {
        tracing::warn!(message_id = %message_id, "Quota precheck rejected: {}", reason);
        return Err(reason);
    }

    // ── SafetyBridge 扫描：密钥检测 + PII 脱敏 ────────────────────
    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        "send_chat_message.scan_user_input.start"
    );
    let scan_result = state.safety_bridge.scan_user_input(&content);
    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        had_sensitive_data = scan_result.had_sensitive_data,
        was_blocked = scan_result.was_blocked,
        "send_chat_message.scan_user_input.end"
    );

    reject_blocked_scan(&scan_result, &message_id, &thread_id)?;

    let dlp_redacted_stats = dlp_stats
        .map(|stats| {
            serde_json::json!({
                "total_matches": stats.total_matches,
                "redacted_count": stats.redacted_count,
                "blocked_count": stats.blocked_count,
                "warned_count": stats.warned_count,
            })
        })
        .or_else(|| {
            if !scan_result.had_sensitive_data {
                return None;
            }
            let secret_count = u32::from(scan_result.stats.secret_detected);
            Some(serde_json::json!({
                "total_matches": scan_result.stats.pii_matches + secret_count as usize,
                "redacted_count": scan_result.stats.redacted_count,
                "blocked_count": scan_result.stats.blocked_count + secret_count as usize,
                "warned_count": scan_result.stats.warned_count,
            }))
        });

    let safe_content = if scan_result.had_sensitive_data {
        tracing::debug!(
            message_id = %message_id,
            pii_matches = scan_result.stats.pii_matches,
            "Message sanitized by SafetyBridge"
        );
        scan_result.sanitized_content
    } else {
        content
    };

    // ── 模型切换 ──────────────────────────────────────────────────
    // 命令（以 / 开头）不需要 LLM，跳过模型切换避免 api_key 脱敏值导致失败
    let is_command = safe_content.starts_with('/');
    if !is_command {
        if let Some(ref id) = model_id {
            tracing::info!(
                target: "ironclaw::startup_latency",
                message_id = %message_id,
                "send_chat_message.model_switch.start"
            );
            let switch_kind = classify_switch(state, api_base_url.as_deref());
            tracing::info!(
                message_id = %message_id,
                model_id = %id,
                api_base_url = ?api_base_url,
                switch_kind = ?switch_kind,
                current_provider_url = %state.provider_base_url.read().map(|g| g.clone()).unwrap_or_default(),
                initial_base_url = %state.initial_base_url,
                active_model = %state.llm.active_model_name(),
                "Model switch decision"
            );
            match switch_kind {
                SwitchKind::InPlace => switch_model_in_place(state, id),
                SwitchKind::CrossProvider => {
                    switch_provider(state, id, api_base_url.as_deref(), api_key.as_deref())?;
                }
                SwitchKind::RestoreInitial => {
                    restore_initial_provider(state, id);
                }
            }
            tracing::info!(
                message_id = %message_id,
                active_model_after = %state.llm.active_model_name(),
                "Model switch complete"
            );
            tracing::info!(
                target: "ironclaw::startup_latency",
                message_id = %message_id,
                "send_chat_message.model_switch.end"
            );
        }
    }

    // ── Skill 激活通知（desktop-client 侧扩展）────────────────────
    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        "send_chat_message.detect_skills.start"
    );
    let activated_skills =
        detect_and_emit_skills_activated(&app_handle, state, &thread_id, &safe_content);
    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        activated = activated_skills.len(),
        "send_chat_message.detect_skills.end"
    );

    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        "send_chat_message.record_user_message.start"
    );
    state.conversation_tracker.record_user_message(
        &thread_id,
        &safe_content,
        scan_result.had_sensitive_data,
        &report_attachments,
    );
    state
        .conversation_tracker
        .record_activated_skills(&thread_id, &activated_skills);
    tracing::info!(
        target: "ironclaw::startup_latency",
        message_id = %message_id,
        "send_chat_message.record_user_message.end"
    );

    let mut msg = IncomingMessage::new("tauri", &state.scope_id, &safe_content)
        .with_thread(&thread_id)
        .with_owner_id(&state.scope_id);

    if !incoming_attachments.is_empty() {
        msg = msg.with_attachments(incoming_attachments);
    }

    let metadata = build_message_metadata(state, dlp_redacted_stats);
    if !metadata.is_null() {
        msg = msg.with_metadata(metadata);
    }

    let send_span = tracing::info_span!(
        target: "ironclaw::startup_latency",
        "send_chat_message.msg_sender_send",
        message_id = %message_id,
    );
    state
        .msg_sender
        .send(msg)
        .instrument(send_span)
        .await
        .map_err(|e| format!("Failed to send message to agent: {}", e))?;

    tracing::debug!(
        message_id = %message_id,
        thread_id = %thread_id,
        model_id = ?model_id,
        "Message injected into agent loop"
    );

    Ok(SendMessageResponse {
        message_id,
        success: true,
    })
}

pub(crate) fn reject_blocked_scan(
    scan_result: &BridgeScanResult,
    message_id: &str,
    thread_id: &str,
) -> Result<(), String> {
    if !scan_result.was_blocked {
        return Ok(());
    }

    tracing::warn!(
        message_id = %message_id,
        thread_id = %thread_id,
        "Message blocked by SafetyBridge"
    );
    Err(scan_result
        .block_reason
        .clone()
        .unwrap_or_else(|| "消息包含敏感信息，已被安全策略拦截".to_string()))
}

#[tauri::command]
pub async fn ic_interrupt_thread(
    state: State<'_, EngineState>,
    thread_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    send_thread_control_message(state, &thread_id, "/interrupt").await
}

#[tauri::command]
pub async fn ic_finalize_thread(
    state: State<'_, EngineState>,
    thread_id: String,
) -> Result<(), String> {
    let state = state.get()?;
    state
        .conversation_tracker
        .finish_thread(&thread_id, &state.data_reporter);

    if let Err(error) = state.data_reporter.flush().await {
        tracing::warn!(thread_id = %thread_id, error = %error, "Failed to flush finalized conversation immediately");
    }

    Ok(())
}

async fn build_attachments(
    attachments: Option<Vec<FrontendAttachment>>,
    scanner: &AttachmentScanner,
) -> Result<(Vec<IncomingAttachment>, Vec<ConversationAttachment>), String> {
    let mut incoming_attachments = Vec::new();
    let mut report_attachments = Vec::new();

    for attachment in attachments.unwrap_or_default() {
        let kind = match attachment.kind.as_str() {
            "audio" => AttachmentKind::Audio,
            "image" => AttachmentKind::Image,
            "document" => AttachmentKind::Document,
            _ => AttachmentKind::from_mime_type(&attachment.mime_type),
        };

        // ── Attachment DLP gate (issue #92, ADR-148 §6.4) ─────
        //
        // Every attachment's `extracted_text` is routed through the
        // project-wide `EgressGate` chain on **both** the LlmRequest
        // and Persistence kinds before it reaches the agent loop or
        // the audit reporter. Binary MIME types are checked against a
        // small whitelist — anything else is rejected (fail-safe).
        let has_binary_payload = !attachment.data.is_empty();
        let decision = scanner
            .scan(
                &attachment.mime_type,
                attachment.extracted_text.as_deref(),
                has_binary_payload,
            )
            .await;

        let extracted_text = match decision {
            AttachmentDecision::Allow => attachment.extracted_text.clone(),
            AttachmentDecision::Redact {
                sanitized_text,
                kinds_redacted,
            } => {
                tracing::info!(
                    attachment_id = %attachment.id,
                    mime = %attachment.mime_type,
                    kinds = ?kinds_redacted,
                    "attachment text redacted by EgressGate",
                );
                Some(sanitized_text)
            }
            AttachmentDecision::Block { reason } => {
                tracing::warn!(
                    attachment_id = %attachment.id,
                    mime = %attachment.mime_type,
                    "attachment blocked by EgressGate: {reason}",
                );
                return Err(format!(
                    "Attachment `{}` was rejected by the DLP gate: {reason}",
                    attachment.filename.as_deref().unwrap_or("(unnamed)")
                ));
            }
        };

        report_attachments.push(ConversationAttachment::from_frontend(
            attachment.id.clone(),
            attachment.kind.clone(),
            attachment.mime_type.clone(),
            attachment.filename.clone(),
            attachment.size_bytes,
            extracted_text.clone(),
            &attachment.data,
            attachment.duration_secs,
        ));

        incoming_attachments.push(IncomingAttachment {
            id: attachment.id,
            kind,
            mime_type: attachment.mime_type,
            filename: attachment.filename,
            size_bytes: attachment.size_bytes,
            source_url: None,
            storage_key: None,
            extracted_text,
            data: attachment.data,
            duration_secs: attachment.duration_secs,
        });
    }

    Ok((incoming_attachments, report_attachments))
}

async fn send_thread_control_message(
    state: &crate::state::AppState,
    thread_id: &str,
    content: &str,
) -> Result<(), String> {
    let msg = IncomingMessage::new("tauri", &state.scope_id, content)
        .with_thread(thread_id)
        .with_owner_id(&state.scope_id);

    state
        .msg_sender
        .send(msg)
        .await
        .map_err(|e| format!("Failed to send thread control message: {}", e))
}

fn build_message_metadata(
    state: &crate::state::AppState,
    dlp_redacted_stats: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();

    if let Some(stats) = dlp_redacted_stats {
        metadata.insert("dlp_redacted_stats".to_string(), stats);
    }

    if let Ok(disabled_skills) = state.disabled_skills_snapshot() {
        metadata.insert(
            "disabled_skills".to_string(),
            serde_json::Value::Array(
                disabled_skills
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    if let Ok(disabled_extensions) = state.disabled_extensions_snapshot() {
        metadata.insert(
            "disabled_extensions".to_string(),
            serde_json::Value::Array(
                disabled_extensions
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }

    if metadata.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::Object(metadata)
    }
}

/// 在消息发送前做 skill 关键词匹配，若有激活则 emit `skills_activated` 事件。
///
/// 使用 ironclaw 已有的 `prefilter_skills` 函数，不修改 ironclaw 任何代码。
/// 失败时静默跳过（skill 通知是 best-effort，不影响消息发送）。
fn detect_and_emit_skills_activated(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState,
    thread_id: &str,
    content: &str,
) -> Vec<String> {
    let Some(registry) = state.skill_registry.as_ref() else {
        return Vec::new();
    };
    let Ok(guard) = registry.read() else {
        return Vec::new();
    };

    // prefilter_skills 需要 &[LoadedSkill]，这里构建已启用技能快照。
    let enabled_skills: Vec<ironclaw::skills::LoadedSkill> = guard
        .skills()
        .iter()
        .filter(|skill| state.skill_enabled(skill.name()))
        .cloned()
        .collect();
    if enabled_skills.is_empty() {
        return Vec::new();
    }

    let skills_cfg = &state.skills_config;
    let selected = ironclaw::skills::prefilter_skills(
        content,
        &enabled_skills,
        skills_cfg.max_active_skills,
        skills_cfg.max_context_tokens,
    );

    if selected.is_empty() {
        return Vec::new();
    }

    let skill_names: Vec<String> = selected.iter().map(|s| s.name().to_string()).collect();
    tracing::debug!(skills = ?skill_names, "Skills activated (client-side detection)");

    let event = VercelUIStream::DataCustom {
        id: None,
        data: serde_json::json!({
            "type": "skills_activated",
            "skills": skill_names,
        }),
    };
    let thread_scope = if thread_id.is_empty() {
        None
    } else {
        Some(thread_id)
    };
    let _ = crate::tauri_channel::emit_chat_stream(app_handle, thread_scope, &event);

    skill_names
}

/// 订阅聊天事件（兼容命令）。
#[tauri::command]
pub async fn subscribe_chat_events(app_handle: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!("subscribe_chat_events called");

    if app_handle
        .try_state::<crate::state::EngineState>()
        .map_or(false, |es| es.is_ready())
    {
        // 系统级广播：前端所有 Transport 透传
        crate::tauri_channel::emit_chat_stream(
            &app_handle,
            None,
            &VercelUIStream::DataCustom {
                id: None,
                data: serde_json::json!({
                    "type": "connection_status",
                    "connected": true,
                    "message": "IronClaw engine ready",
                }),
            },
        )
        .map_err(|e| format!("Failed to emit connection status: {}", e))?;
    }

    Ok(())
}

/// 取消订阅聊天事件（兼容命令，no-op）。
#[tauri::command]
pub async fn unsubscribe_chat_events() -> Result<(), String> {
    tracing::debug!("unsubscribe_chat_events called (no-op in embedded mode)");
    Ok(())
}

/// 引擎就绪状态快照（可查询）。
///
/// 三态对应 [`EngineState`]：`Starting`（`ready=false, failed=false`）、
/// `Ready`（`ready=true`）、`Failed`（`failed=true` 且 `error` 填充）。
#[derive(Debug, Clone, Serialize)]
pub struct EngineStatus {
    /// 引擎是否已就绪。
    pub ready: bool,
    /// 引擎是否启动失败。
    pub failed: bool,
    /// 启动失败原因（仅 `failed` 为 true 时填充）。
    pub error: Option<String>,
}

/// 查询当前引擎就绪状态。
///
/// 与一次性的 `connection_status` 广播事件互补：广播负责"启动 → 就绪"
/// 的实时通知；本命令负责"前端任意时刻（含 webview 刷新后）查询当前状态"，
/// 从根上消除"刷新后错过一次性事件 → 永久卡在引擎启动中"的问题。
#[tauri::command]
pub async fn get_engine_status(engine: State<'_, EngineState>) -> Result<EngineStatus, String> {
    let failed = engine.is_failed();
    let error = if failed { engine.get().err() } else { None };
    Ok(EngineStatus {
        ready: engine.is_ready(),
        failed,
        error,
    })
}

// ── 模型切换辅助函数 ─────────────────────────────────────────────

/// 模型切换类型。
#[derive(Debug)]
enum SwitchKind {
    /// 模型在当前 provider 内，用 set_model 切换
    InPlace,
    /// 模型需要不同的 base_url，创建新 provider
    CrossProvider,
    /// 模型回到初始 provider（恢复 .env 配置的 provider）
    RestoreInitial,
}

/// 根据目标模型的 api_base_url 判断切换类型。
fn classify_switch(state: &crate::state::AppState, api_base_url: Option<&str>) -> SwitchKind {
    // api_base_url 为空时，目标是初始 provider（DashScope 等 .env 配置的 provider）
    let new_url = match api_base_url.filter(|u| !u.is_empty()) {
        Some(u) => normalize_base_url(u),
        None => normalize_base_url(&state.initial_base_url),
    };

    let current = state
        .provider_base_url
        .read()
        .map(|g| g.clone())
        .unwrap_or_default();

    let initial = normalize_base_url(&state.initial_base_url);

    // 容错比较：`https://host/v1` 和 `https://host` 视为同一 provider
    let urls_match = |a: &str, b: &str| -> bool {
        a == b || a.trim_end_matches("/v1") == b.trim_end_matches("/v1")
    };

    if urls_match(&new_url, &current) {
        SwitchKind::InPlace
    } else if !initial.is_empty() && urls_match(&new_url, &initial) {
        SwitchKind::RestoreInitial
    } else {
        SwitchKind::CrossProvider
    }
}

/// 跨 provider 切换：用新的 base_url + api_key 重建 provider。
pub(crate) fn switch_provider(
    state: &crate::state::AppState,
    model_id: &str,
    api_base_url: Option<&str>,
    api_key: Option<&str>,
) -> Result<(), String> {
    let base_url = api_base_url
        .filter(|u| !u.is_empty())
        .ok_or("跨 provider 切换需要 api_base_url")?;

    // api_key 为 "****" 时表示前端返回的脱敏值，从本地存储取真实 key。
    let resolved_key: String;
    let key = match api_key.filter(|k| !k.is_empty() && *k != "****") {
        Some(k) => k,
        None => {
            resolved_key =
                crate::ipc::models::lookup_custom_api_key(model_id).ok_or_else(|| {
                    format!("模型 {} 的 API key 未找到，请在设置中重新配置", model_id)
                })?;
            &resolved_key
        }
    };

    // 规范化 base_url：去掉尾部的 /chat/completions 等路径，
    // rig-core 会自动拼接 /chat/completions。
    let normalized_url = normalize_base_url(base_url);

    let before = state.llm.active_model_name();

    let new_provider = ironclaw::llm::create_openai_provider(&normalized_url, key, model_id)
        .map_err(|e| format!("创建新 provider 失败: {}", e))?;

    state.model_switch.replace_inner(new_provider);

    if let Ok(mut guard) = state.provider_base_url.write() {
        *guard = normalized_url.clone();
    }

    tracing::info!(
        model = %model_id,
        base_url = %normalized_url,
        before = %before,
        key_len = key.len(),
        "Provider switched (cross-provider)"
    );
    Ok(())
}

/// 规范化 API base URL：去掉 rig-core 会自动拼接的路径后缀。
///
/// rig-core 的 `CompletionsClient` 发请求时会在 base_url 后拼接 `/chat/completions`，
/// 所以这里只剥掉用户可能多填的 `/chat/completions` 后缀。
///
/// `/v1` 是 base URL 的一部分（如 `https://dashscope.aliyuncs.com/compatible-mode/v1`），
/// 不能剥掉，否则最终 URL 会缺少 `/v1` 路径段导致 404。
///
/// provider 比较时的容错匹配（带 `/v1` vs 不带）由 `classify_switch` 单独处理。
pub(crate) fn normalize_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    for suffix in &["/chat/completions", "/completions"] {
        if let Some(base) = trimmed.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    trimmed.to_string()
}

/// 恢复到初始 provider（.env 配置的 provider）。
///
/// 跨 provider 切换后，用户选回初始 provider 的模型时调用。
/// 用保存的初始 provider 引用恢复，然后 set_model 切换模型名。
fn restore_initial_provider(state: &crate::state::AppState, model_id: &str) {
    state
        .model_switch
        .replace_inner(Arc::clone(&state.initial_provider));

    if let Ok(mut guard) = state.provider_base_url.write() {
        *guard = state.initial_base_url.clone();
    }

    // 在恢复的初始 provider 上切换模型名
    switch_model_in_place(state, model_id);

    tracing::info!(
        model = %model_id,
        base_url = %state.initial_base_url,
        "Restored to initial provider"
    );
}

/// 同 provider 内模型切换：set_model 优先，回退到 per-request override。
fn switch_model_in_place(state: &crate::state::AppState, model_id: &str) {
    let before = state.llm.active_model_name();
    match state.llm.set_model(model_id) {
        Ok(()) => {
            if let Ok(mut guard) = state.model_override.write() {
                *guard = None;
            }
            tracing::debug!(
                requested = %model_id, before = %before,
                after = %state.llm.active_model_name(),
                "Model switched via set_model"
            );
        }
        Err(_) => {
            if let Ok(mut guard) = state.model_override.write() {
                *guard = Some(model_id.to_string());
            }
            tracing::debug!(requested = %model_id, "Using per-request model override");
        }
    }
}

// ── 配额预检 ─────────────────────────────────────────────────────

/// 调用 Admin Backend 的 /api/quota/check 接口进行费用预检。
///
/// Fail-Safe 设计：如果预检接口不可用（网络错误、超时），拒绝请求。
/// 政企场景下安全优先，宁可暂时不可用也不能超额使用。
#[tracing::instrument(target = "ironclaw::startup_latency", skip(state))]
async fn quota_precheck(state: &crate::state::AppState) -> Result<(), String> {
    let admin_url = std::env::var("ADMIN_API_URL").unwrap_or_default();
    if admin_url.is_empty() {
        // 未配置 Admin Backend URL，跳过预检（本地开发模式）
        return Ok(());
    }

    let user_id = state.require_backend_user_id("配额预检")?;
    let url = format!("{}/api/quota/check", admin_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("配额服务不可用: {}", e))?;

    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "user_id": user_id }))
        .send()
        .await
        .map_err(|_| "配额服务暂时不可用，请稍后重试".to_string())?;

    if !resp.status().is_success() {
        return Err("配额服务暂时不可用，请稍后重试".to_string());
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|_| "配额服务响应异常".to_string())?;

    if body["allowed"].as_bool() == Some(false) {
        let reason = body["reason"].as_str().unwrap_or("当日费用已达限额");
        return Err(reason.to_string());
    }

    Ok(())
}

pub(crate) fn usage_report_backend_user_id(
    backend_user_id: &std::sync::RwLock<Option<uuid::Uuid>>,
) -> Result<Option<uuid::Uuid>, String> {
    backend_user_id
        .read()
        .map(|guard| *guard)
        .map_err(|_| "后台用户身份读取失败，跳过费用上报".to_string())
}

// ── 费用上报 ─────────────────────────────────────────────────────

/// 向 Admin Backend 上报 LLM 调用的 Token 消耗。
///
/// 上报失败不阻塞主流程，仅记录警告日志。
/// 后端根据模型单价计算实际费用并写入 usage_records 表。
pub async fn report_usage_to_admin(
    state: &crate::state::AppState,
    model_id: &str,
    input_tokens: u32,
    output_tokens: u32,
) {
    let admin_url = std::env::var("ADMIN_API_URL").unwrap_or_default();
    if admin_url.is_empty() {
        return; // 未配置后台，跳过上报
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("创建 HTTP 客户端失败: {}", e);
            return;
        }
    };

    let url = format!("{}/api/quota/report-usage", admin_url);
    let user_id = match usage_report_backend_user_id(&state.backend_user_id) {
        Ok(Some(user_id)) => user_id,
        Ok(None) => {
            tracing::warn!("后台用户身份尚未就绪，跳过费用上报");
            return;
        }
        Err(error) => {
            tracing::warn!("{}", error);
            return;
        }
    };

    if user_id.is_nil() {
        tracing::warn!("后台用户身份尚未就绪，跳过费用上报");
        return;
    }

    let payload = serde_json::json!({
        "user_id": user_id,
        "model_id": model_id,
        "input_tokens": input_tokens as i64,
        "output_tokens": output_tokens as i64,
    });

    match client.post(&url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!(
                model = %model_id,
                input = input_tokens,
                output = output_tokens,
                "费用上报成功"
            );
        }
        Ok(resp) => {
            tracing::warn!(
                status = %resp.status(),
                "费用上报失败"
            );
        }
        Err(e) => {
            tracing::warn!("费用上报请求失败: {}", e);
        }
    }
}

/// 立即激活指定模型，不需要发送消息。
///
/// 解决"切换模型后定时任务仍用旧 provider"的问题：
/// 原来模型切换只在 send_chat_message 时触发，现在选择模型时立即调用此命令。
#[tauri::command]
pub async fn ic_activate_model(
    state: State<'_, EngineState>,
    model_id: String,
    api_base_url: Option<String>,
    api_key: Option<String>,
) -> Result<(), String> {
    let state = state.get()?;
    let switch_kind = classify_switch(state, api_base_url.as_deref());
    match switch_kind {
        SwitchKind::InPlace => switch_model_in_place(state, &model_id),
        SwitchKind::CrossProvider => {
            switch_provider(
                state,
                &model_id,
                api_base_url.as_deref(),
                api_key.as_deref(),
            )?;
        }
        SwitchKind::RestoreInitial => {
            restore_initial_provider(state, &model_id);
        }
    }
    tracing::info!(
        model = %model_id,
        active = %state.llm.active_model_name(),
        "Model activated immediately via ic_activate_model"
    );
    Ok(())
}
