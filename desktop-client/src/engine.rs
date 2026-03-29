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

use crate::state::{AppState, EngineState};
use crate::model_switch::ModelSwitchProvider;
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

    // ── Phase 1.5: 种植内置 Skills ────────────────────────────────
    // 在 AppBuilder 之前执行，确保 discover_all() 能找到内置 skills。
    // 利用 ironclaw 已有的 installed_dir 机制，不修改 ironclaw 任何代码。
    seed_builtin_skills(&app_handle, &config.skills.installed_dir).await;

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

    // ── 模型切换 ──────────────────────────────────────────────────
    // model_override 在 AppState 和 ModelSwitchProvider 之间共享。
    let model_override = Arc::new(std::sync::RwLock::new(None::<String>));
    let model_switch = Arc::new(
        ModelSwitchProvider::new(Arc::clone(&components.llm), Arc::clone(&model_override)),
    );
    let wrapped_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::clone(&model_switch) as _;

    // 记录初始 provider 的 base URL（用于跨 provider 切换检测）。
    let initial_base_url = config
        .llm
        .provider
        .as_ref()
        .map(|p| p.base_url.clone())
        .unwrap_or_default();

    let app_state = AppState {
        msg_sender,
        db: components.db.clone(),
        workspace: components.workspace.clone(),
        tools: Arc::clone(&components.tools),
        extension_manager: components.extension_manager.clone(),
        skill_registry: components.skill_registry.clone(),
        skill_catalog: components.skill_catalog.clone(),
        skills_config: config.skills.clone(),
        safety: Arc::clone(&components.safety),
        safety_bridge,
        context_manager: Arc::clone(&components.context_manager),
        owner_id: config.owner_id.clone(),
        llm: Arc::clone(&wrapped_llm),
        model_override: Arc::clone(&model_override),
        model_switch: Arc::clone(&model_switch),
        provider_base_url: std::sync::RwLock::new(initial_base_url.clone()),
        initial_provider: Arc::clone(&components.llm),
        initial_base_url,
    };
    // 从 Tauri managed state 获取 EngineState 并填充
    let engine_state = app_handle.state::<EngineState>();
    engine_state
        .initialize(app_state)
        .map_err(|e| anyhow::anyhow!(e))?;

    tracing::info!("AppState injected into EngineState");

    // ── 启动时同步 Admin Backend DLP 规则 ─────────────────────────
    // 非阻塞：同步失败不影响引擎启动，仅使用内置规则
    {
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            // 等待引擎完全就绪后再同步
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let engine_state = app_handle_clone.state::<EngineState>();
            match engine_state.get() {
                Ok(state) => {
                    match crate::ipc::dlp::do_sync_dlp_rules(&state.safety_bridge).await {
                        Ok(result) => tracing::info!(
                            rules = result.rules_synced,
                            "DLP rules synced from admin backend on startup"
                        ),
                        Err(e) => tracing::warn!(
                            error = %e,
                            "Failed to sync DLP rules from admin backend (using built-in rules)"
                        ),
                    }
                }
                Err(e) => tracing::warn!("Engine not ready for DLP sync: {}", e),
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
        llm: wrapped_llm,
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
                message: crate::error::friendly_engine_error(&e.to_string()),
                code: Some("ENGINE_ERROR".into()),
            },
        );
        return Err(e.into());
    }

    tracing::info!("IronClaw engine shut down gracefully");
    Ok(())
}

// ── 内部辅助函数 ──────────────────────────────────────────────────

/// 将内置 skills 种植到 ironclaw 的 `installed_dir`。
///
/// 在 `Config::from_env()` 之后调用，此时 `installed_dir` 路径已确定。
/// 对每个内置 SKILL.md，若目标目录中不存在同名 skill，则复制过去。
/// 已存在的 skill 不覆盖（用户可以卸载内置 skill）。
///
/// 这是纯 desktop-client 侧的扩展，不修改 ironclaw 任何代码。
/// ironclaw 的 `installed_dir` 机制本来就支持外部写入，这里只是利用它。
pub(crate) async fn seed_builtin_skills(app_handle: &AppHandle, installed_dir: &std::path::Path) {
    let Some(source_dir) = find_builtin_skills_source(app_handle) else {
        tracing::debug!("No builtin skills source directory found, skipping seed");
        return;
    };

    let mut read_dir = match tokio::fs::read_dir(&source_dir).await {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!("Cannot read builtin skills dir {:?}: {}", source_dir, e);
            return;
        }
    };

    let mut seeded = 0u32;
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let Some(skill_name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };

        let dest_dir = installed_dir.join(&skill_name);
        // 已存在则跳过（不覆盖用户修改或已安装版本）
        if dest_dir.join("SKILL.md").exists() {
            continue;
        }

        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            tracing::warn!("Failed to create skill dir {:?}: {}", dest_dir, e);
            continue;
        }
        if let Err(e) = tokio::fs::copy(&skill_md, dest_dir.join("SKILL.md")).await {
            tracing::warn!("Failed to copy builtin skill {:?}: {}", skill_name, e);
            continue;
        }
        seeded += 1;
        tracing::info!(skill = %skill_name, "Seeded builtin skill to installed_dir");
    }

    if seeded > 0 {
        tracing::info!(count = seeded, "Builtin skills seeded");
    }
}

/// 查找内置 skills 源目录（按优先级）。
fn find_builtin_skills_source(app_handle: &AppHandle) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();

    // 1. Tauri resource dir（打包后）
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join("skills"));
    }
    // 2. 开发时相对路径（从项目根目录运行）
    candidates.push(std::path::PathBuf::from("ironclaw/skills"));
    // 3. 开发时相对路径（从 desktop-client/ 目录运行，即 `cargo tauri dev` 的 CWD）
    candidates.push(std::path::PathBuf::from("../ironclaw/skills"));
    // 4. 可执行文件目录向上查找（CI / 非标准工作目录）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("../../../ironclaw/skills"));
        }
    }

    candidates
        .into_iter()
        .find(|p| p.is_dir())
        .map(|p| p.canonicalize().unwrap_or(p))
}
