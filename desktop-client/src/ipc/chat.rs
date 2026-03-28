//! 聊天相关 Tauri Commands。
//!
//! 通过 `AppState.msg_sender` 将用户消息注入 TauriChannel，
//! Agent 处理后通过 `ChatEvent` 推送回复到前端。
//!
//! # 前端兼容性
//!
//! 命令名与现有前端 `useAiChatTauri.ts` 完全匹配：
//! - `send_chat_message` — 发送消息
//! - `subscribe_chat_events` — 订阅事件（新架构下为 no-op）
//! - `unsubscribe_chat_events` — 取消订阅（新架构下为 no-op）

use ironclaw::channels::IncomingMessage;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use crate::state::EngineState;
use crate::tauri_channel::ChatEvent;

/// 发送消息的响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub success: bool,
}

/// 发送聊天消息。
///
/// 构造 `IncomingMessage` 并通过 `msg_sender` 注入 Agent 消息循环。
/// AI 回复通过 `chat-event` Tauri 事件异步推送到前端。
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
/// 若有 skill 被激活，通过 `chat-event` 发出 `skills_activated` 事件。
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
) -> Result<SendMessageResponse, String> {
    let state = state.get()?;
    let message_id = uuid::Uuid::new_v4().to_string();

    // ── SafetyBridge 扫描：密钥检测 + PII 脱敏 ────────────────────
    let scan_result = state.safety_bridge.scan_user_input(&content);

    if scan_result.was_blocked {
        tracing::warn!(
            message_id = %message_id,
            thread_id = %thread_id,
            "Message blocked by SafetyBridge"
        );
        return Err(scan_result.block_reason.unwrap_or_else(|| {
            "消息包含敏感信息，已被安全策略拦截".to_string()
        }));
    }

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
    // set_model() 优先；不支持时回退到 per-request override。
    if let Some(ref id) = model_id {
        let before = state.llm.active_model_name();
        match state.llm.set_model(id) {
            Ok(()) => {
                if let Ok(mut guard) = state.model_override.write() {
                    *guard = None;
                }
                tracing::debug!(
                    requested = %id, before = %before,
                    after = %state.llm.active_model_name(),
                    "Model switched via set_model"
                );
            }
            Err(_) => {
                if let Ok(mut guard) = state.model_override.write() {
                    *guard = Some(id.clone());
                }
                tracing::debug!(requested = %id, "Using per-request model override");
            }
        }
    }

    // ── Skill 激活通知（desktop-client 侧扩展）────────────────────
    emit_skills_activated_if_any(&app_handle, state, &safe_content);

    let msg = IncomingMessage::new("tauri", &state.owner_id, &safe_content)
        .with_thread(&thread_id)
        .with_owner_id(&state.owner_id);

    state
        .msg_sender
        .send(msg)
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

/// 在消息发送前做 skill 关键词匹配，若有激活则 emit `skills_activated` 事件。
///
/// 使用 ironclaw 已有的 `prefilter_skills` 函数，不修改 ironclaw 任何代码。
/// 失败时静默跳过（skill 通知是 best-effort，不影响消息发送）。
fn emit_skills_activated_if_any(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState,
    content: &str,
) {

    let Some(registry) = state.skill_registry.as_ref() else {
        return;
    };
    let Ok(guard) = registry.read() else {
        return;
    };

    let skills_cfg = &state.skills_config;
    let selected = ironclaw::skills::prefilter_skills(
        content,
        guard.skills(),
        skills_cfg.max_active_skills,
        skills_cfg.max_context_tokens,
    );

    if selected.is_empty() {
        return;
    }

    let skill_names: Vec<String> = selected.iter().map(|s| s.name().to_string()).collect();
    tracing::debug!(skills = ?skill_names, "Skills activated (client-side detection)");

    let _ = app_handle.emit(
        "chat-event",
        crate::tauri_channel::ChatEvent::SkillsActivated { skills: skill_names },
    );
}

/// 订阅聊天事件（兼容命令）。
#[tauri::command]
pub async fn subscribe_chat_events(app_handle: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!("subscribe_chat_events called");

    if app_handle
        .try_state::<crate::state::EngineState>()
        .map_or(false, |es| es.is_ready())
    {
        app_handle
            .emit(
                "chat-event",
                ChatEvent::ConnectionStatus {
                    connected: true,
                    message: "IronClaw engine ready".to_string(),
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
