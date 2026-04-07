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

use std::sync::Arc;

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
    api_base_url: Option<String>,
    api_key: Option<String>,
) -> Result<SendMessageResponse, String> {
    let state = state.get()?;
    let message_id = uuid::Uuid::new_v4().to_string();

    // ── 配额预检：调用 Admin Backend 检查是否超额 ─────────────────
    if let Err(reason) = quota_precheck(state).await {
        tracing::warn!(message_id = %message_id, "Quota precheck rejected: {}", reason);
        return Err(reason);
    }

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
    if let Some(ref id) = model_id {
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

    if new_url == current {
        SwitchKind::InPlace
    } else if !initial.is_empty() && new_url == initial {
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
    let key = api_key
        .filter(|k| !k.is_empty() && *k != "****")
        .ok_or("跨 provider 切换需要有效的 api_key（非脱敏值）")?;

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
/// 与 `admin-backend/src/routes.rs` 中的 `normalize_api_base_url` 职责不同：
/// - admin 端：保存时剥离 SDK 自动拼接的路径（`/chat/completions`、`/v1/messages`），
///   但保留 `/v1`（它是 base URL 的一部分，不是 SDK 拼接的）。
/// - 客户端：provider 比较时额外剥离 `/v1`，用于容错匹配
///   （.env 配置可能带 `/v1`，admin 配置可能不带，两者应视为同一 provider）。
pub(crate) fn normalize_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    for suffix in &[
        "/v1/chat/completions",
        "/chat/completions",
        "/completions",
        "/v1",
    ] {
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
    state.model_switch.replace_inner(Arc::clone(&state.initial_provider));

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
async fn quota_precheck(state: &crate::state::AppState) -> Result<(), String> {
    let admin_url = std::env::var("ADMIN_API_URL").unwrap_or_default();
    if admin_url.is_empty() {
        // 未配置 Admin Backend URL，跳过预检（本地开发模式）
        return Ok(());
    }

    let user_id = &state.owner_id;
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
        let reason = body["reason"]
            .as_str()
            .unwrap_or("当日费用已达限额");
        return Err(reason.to_string());
    }

    Ok(())
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
    let payload = serde_json::json!({
        "user_id": state.owner_id,
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
            switch_provider(state, &model_id, api_base_url.as_deref(), api_key.as_deref())?;
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
