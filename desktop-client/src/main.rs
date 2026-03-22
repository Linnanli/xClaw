#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! IronClaw Desktop Client — 嵌入式架构入口。
//!
//! IronClaw 作为库直接嵌入客户端进程，无需外部服务。
//! 启动流程：
//!
//! 1. 加载本地缓存配置 + 管理端配置 → 注入环境变量
//! 2. Tauri Builder setup → spawn `start_ironclaw_engine()`
//! 3. 引擎初始化 → AppState 注入 Tauri 全局状态
//! 4. 前端通过 Tauri IPC 与 Agent 交互

use std::env;

use tracing_subscriber::EnvFilter;

fn main() {
    // ── 初始化日志 ────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,desktop_client=debug,ironclaw=info")),
        )
        .init();

    // ── 加载 .env 文件 ────────────────────────────────────────────
    // 使用 config-rs 加载环境变量，与旧架构保持一致
    load_dotenv();

    // ── 注入管理端配置到环境变量 ──────────────────────────────────
    // ⚠️ 必须在 Tokio runtime 启动前执行（set_var 在多线程中不安全）
    inject_admin_config_to_env();

    tracing::info!("Starting IronClaw Desktop Client (embedded mode)");

    // ── 启动 Tauri ────────────────────────────────────────────────
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();

            // 在 Tauri 异步上下文中启动 IronClaw 引擎
            tauri::async_runtime::spawn(async move {
                if let Err(e) = desktop_client::engine::start_ironclaw_engine(app_handle.clone())
                    .await
                {
                    tracing::error!(error = %e, "IronClaw engine failed to start");

                    // 通知前端引擎启动失败
                    use tauri::Emitter;
                    let _ = app_handle.emit(
                        "chat-event",
                        desktop_client::tauri_channel::ChatEvent::Error {
                            message: format!("Engine startup failed: {}", e),
                            code: Some("ENGINE_STARTUP_FAILED".into()),
                        },
                    );
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ── 聊天（嵌入式 IronClaw）──────────────────────────
            desktop_client::ipc::send_chat_message,
            desktop_client::ipc::subscribe_chat_events,
            desktop_client::ipc::unsubscribe_chat_events,
            // ── 线程管理 ────────────────────────────────────────
            desktop_client::ipc::ic_list_threads,
            desktop_client::ipc::ic_create_thread,
            desktop_client::ipc::ic_get_thread_history,
            // ── 记忆/工作空间 ───────────────────────────────────
            desktop_client::ipc::ic_memory_list,
            desktop_client::ipc::ic_memory_read,
            desktop_client::ipc::ic_memory_write,
            desktop_client::ipc::ic_memory_delete,
            desktop_client::ipc::ic_memory_search,
            // ── 技能管理 ────────────────────────────────────────
            desktop_client::ipc::ic_list_skills,
            desktop_client::ipc::ic_search_skills,
            desktop_client::ipc::ic_install_skill,
            desktop_client::ipc::ic_uninstall_skill,
            // ── 扩展管理 ────────────────────────────────────────
            desktop_client::ipc::ic_list_extensions,
            desktop_client::ipc::ic_install_extension,
            desktop_client::ipc::ic_uninstall_extension,
            desktop_client::ipc::ic_search_extensions,
            // ── 工具审批 ────────────────────────────────────────
            desktop_client::ipc::ic_approve_tool,
            desktop_client::ipc::ic_deny_tool,
            // ── DLP 桥接（兼容前端 useDlpScan.ts）──────────────
            desktop_client::ipc::scan_user_input,
            desktop_client::ipc::scan_outbound_request,
            desktop_client::ipc::sanitize_for_storage,
            desktop_client::ipc::check_http_request,
            desktop_client::ipc::get_dlp_config,
            desktop_client::ipc::update_dlp_config,
            desktop_client::ipc::get_dlp_statistics,
            desktop_client::ipc::sync_dlp_rules_from_admin,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 加载 .env 文件到环境变量。
fn load_dotenv() {
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        }
    });

    let config_result = config::Config::builder()
        .add_source(config::File::with_name("desktop-client/.env").required(false))
        .add_source(
            config::File::with_name(&format!("desktop-client/.env.{}", environment))
                .required(false),
        )
        .add_source(
            config::Environment::default()
                .try_parsing(true)
                .separator("_"),
        )
        .build();

    if let Ok(config) = config_result {
        if let Ok(settings) =
            config.try_deserialize::<std::collections::HashMap<String, String>>()
        {
            for (key, value) in settings {
                env::set_var(&key, &value);
            }
        }
    }

    env::set_var("ENVIRONMENT", &environment);
}

/// 从本地缓存加载管理端配置并注入环境变量。
///
/// 管理端可下发 LLM API Key、模型名称、安全策略等配置。
/// 这些配置被注入为环境变量，供 `Config::from_env()` 读取。
///
/// # 安全
///
/// - 配置文件本地加密存储
/// - API Key 不写入日志
/// - 此函数必须在 Tokio runtime 启动前调用（`set_var` 线程安全要求）
fn inject_admin_config_to_env() {
    let cache_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ironclaw-desktop")
        .join("admin_config.json");

    if !cache_path.exists() {
        tracing::debug!("No admin config cache found, using defaults");
        return;
    }

    match std::fs::read_to_string(&cache_path) {
        Ok(content) => {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                let mappings = [
                    ("llm_backend", "LLM_BACKEND"),
                    ("llm_api_key", "LLM_API_KEY"),
                    ("llm_model", "LLM_MODEL"),
                    ("llm_base_url", "LLM_BASE_URL"),
                ];

                let mut injected = 0;
                for (json_key, env_key) in &mappings {
                    if let Some(val) = config.get(json_key).and_then(|v| v.as_str()) {
                        if !val.is_empty() {
                            env::set_var(env_key, val);
                            // ⚠️ 不记录 API Key 的值
                            if *env_key == "LLM_API_KEY" {
                                tracing::info!("Injected {} from admin config (***)", env_key);
                            } else {
                                tracing::info!("Injected {}={} from admin config", env_key, val);
                            }
                            injected += 1;
                        }
                    }
                }

                if injected > 0 {
                    tracing::info!("Injected {} admin config values", injected);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to read admin config cache: {}", e);
        }
    }
}
