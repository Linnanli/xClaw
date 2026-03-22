//! IronClaw 引擎启动逻辑。
//!
//! 负责在 Tauri 进程内初始化 IronClaw 的全部组件，
//! 创建 `TauriChannel`，构建 `Agent`，并启动消息循环。
//!
//! # 启动时序
//!
//! 1. `Config::from_env()` — 加载配置（env vars 已由 `admin_sync` 注入）
//! 2. `AppBuilder::build_all()` — 初始化 DB、LLM、Tools、Extensions 等
//! 3. 创建 `TauriChannel` + `ChannelManager`
//! 4. 构建 `AgentDeps` + `Agent::new()`
//! 5. `app_handle.manage(AppState)` — 注入 Tauri 全局状态
//! 6. `agent.run()` — 阻塞运行消息循环

use std::sync::Arc;

use anyhow::Context;
use tauri::{AppHandle, Manager};
use tracing;

use ironclaw::agent::{Agent, AgentDeps};
use ironclaw::app::{AppBuilder, AppBuilderFlags};
use ironclaw::channels::ChannelManager;
use ironclaw::channels::web::log_layer::LogBroadcaster;
use ironclaw::config::Config;
use ironclaw::hooks::bootstrap_hooks;
use ironclaw::llm::create_session_manager;

use crate::state::AppState;
use crate::safety_bridge::SafetyBridge;
use crate::tauri_channel::{ChatEvent, TauriChannel};

/// 启动 IronClaw 引擎。
///
/// 在 Tauri `setup` 回调中通过 `tauri::async_runtime::spawn` 调用。
/// 引擎启动后，`AppState` 被注入 Tauri 全局状态，前端即可通过
/// Tauri Command 与 Agent 交互。
///
/// # Errors
///
/// 返回 `anyhow::Error`，调用方应捕获并通过 `ChatEvent::Error` 通知前端。
pub async fn start_ironclaw_engine(app_handle: AppHandle) -> anyhow::Result<()> {
    tracing::info!("Starting IronClaw embedded engine...");

    // ── Phase 1: 加载配置 ──────────────────────────────────────────
    let config = Config::from_env()
        .await
        .context("Failed to load IronClaw configuration")?;

    let session = create_session_manager(config.llm.session.clone()).await;
    let log_broadcaster = Arc::new(LogBroadcaster::new());

    tracing::info!(
        backend = %config.llm.backend,
        owner_id = %config.owner_id,
        "Configuration loaded"
    );

    // ── Phase 2: 构建所有组件 ──────────────────────────────────────
    let flags = AppBuilderFlags { no_db: false };
    let components = AppBuilder::new(
        config,
        flags,
        None, // 无 TOML 配置文件
        session.clone(),
        Arc::clone(&log_broadcaster),
    )
    .build_all()
    .await
    .context("Failed to build IronClaw components")?;

    let config = components.config.clone();

    tracing::info!(
        tools = components.tools.count(),
        db = components.db.is_some(),
        workspace = components.workspace.is_some(),
        skills = components.skill_registry.is_some(),
        extensions = components.extension_manager.is_some(),
        "IronClaw components initialized"
    );

    // ── Phase 3: 创建 TauriChannel ────────────────────────────────
    let tauri_channel = TauriChannel::new(app_handle.clone());
    let msg_sender = tauri_channel.sender();

    // ── Phase 4: 注册到 ChannelManager ────────────────────────────
    let channels = ChannelManager::new();
    channels.add(Box::new(tauri_channel)).await;
    let channels = Arc::new(channels);

    // ── Phase 5: 注册生命周期 Hooks ───────────────────────────────
    let active_tool_names = components.tools.list().await;
    let _hook_bootstrap = bootstrap_hooks(
        &components.hooks,
        components.workspace.as_ref(),
        &config.wasm.tools_dir,
        &config.channels.wasm_channels_dir,
        &active_tool_names,
        &[], // 无 WASM channel
        &components.dev_loaded_tool_names,
    )
    .await;

    // ── Phase 6: 注入 Tauri 全局状态 ──────────────────────────────
    let safety_bridge = Arc::new(SafetyBridge::new(
        Arc::clone(&components.safety),
        None, // 使用默认 DLP 脱敏配置
        None, // DataReporter 在 admin_sync 阶段注入
    ));

    let app_state = AppState {
        msg_sender,
        db: components.db.clone(),
        workspace: components.workspace.clone(),
        tools: Arc::clone(&components.tools),
        extension_manager: components.extension_manager.clone(),
        skill_registry: components.skill_registry.clone(),
        skill_catalog: components.skill_catalog.clone(),
        safety: Arc::clone(&components.safety),
        safety_bridge,
        context_manager: Arc::clone(&components.context_manager),
        owner_id: config.owner_id.clone(),
    };
    app_handle.manage(app_state);

    tracing::info!("AppState injected into Tauri");

    // ── 启动时同步 Admin Backend DLP 规则 ─────────────────────────
    // 非阻塞：同步失败不影响引擎启动，仅使用内置规则
    {
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            // 等待引擎完全就绪后再同步
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let state = app_handle_clone.state::<crate::state::AppState>();
            match crate::ipc::dlp::sync_dlp_rules_from_admin(state).await {
                Ok(result) => tracing::info!(
                    rules = result.rules_synced,
                    "DLP rules synced from admin backend on startup"
                ),
                Err(e) => tracing::warn!(
                    error = %e,
                    "Failed to sync DLP rules from admin backend (using built-in rules)"
                ),
            }
        });
    }

    // 通知前端引擎已就绪
    use tauri::Emitter;
    let _ = app_handle.emit(
        "chat-event",
        ChatEvent::ConnectionStatus {
            connected: true,
            message: "IronClaw engine ready".to_string(),
        },
    );

    // ── Phase 7: 注册消息工具 ─────────────────────────────────────
    components
        .tools
        .register_message_tools(Arc::clone(&channels), components.extension_manager.clone())
        .await;

    // ── Phase 8: 构建 Agent ───────────────────────────────────────
    let session_manager = Arc::clone(&components.agent_session_manager);

    let deps = AgentDeps {
        owner_id: config.owner_id.clone(),
        store: components.db,
        llm: components.llm,
        cheap_llm: components.cheap_llm,
        safety: components.safety,
        tools: components.tools,
        workspace: components.workspace,
        extension_manager: components.extension_manager,
        skill_registry: components.skill_registry,
        skill_catalog: components.skill_catalog,
        skills_config: config.skills.clone(),
        hooks: components.hooks,
        cost_guard: components.cost_guard,
        sse_tx: None, // 不使用 SSE — TauriChannel 直接推送
        http_interceptor: components
            .recording_handle
            .as_ref()
            .map(|r| r.http_interceptor()),
        transcription: config
            .transcription
            .create_provider()
            .map(|p| Arc::new(ironclaw::transcription::TranscriptionMiddleware::new(p))),
        document_extraction: Some(Arc::new(
            ironclaw::document_extraction::DocumentExtractionMiddleware::new(),
        )),
    };

    let agent = Agent::new(
        config.agent.clone(),
        deps,
        channels,
        Some(config.heartbeat.clone()),
        Some(config.hygiene.clone()),
        Some(config.routines.clone()),
        Some(components.context_manager),
        Some(session_manager),
    );

    tracing::info!("Agent constructed, starting message loop...");

    // ── Phase 9: 运行 Agent ───────────────────────────────────────
    // agent.run() 阻塞直到所有 channel stream 结束或收到 Ctrl+C。
    if let Err(e) = agent.run().await {
        tracing::error!(error = %e, "Agent exited with error");
        let _ = app_handle.emit(
            "chat-event",
            ChatEvent::Error {
                message: format!("IronClaw engine error: {}", e),
                code: Some("ENGINE_ERROR".into()),
            },
        );
        return Err(e.into());
    }

    tracing::info!("IronClaw engine shut down gracefully");
    Ok(())
}
