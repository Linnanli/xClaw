#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! IronClaw Desktop Client — 嵌入式架构入口。
//!
//! # 配置加载架构
//!
//! 客户端配置与全局 IronClaw CLI (`~/.ironclaw/`) 完全隔离。
//! 所有数据存储在 `~/Library/Application Support/ironclaw-desktop/` 下。
//!
//! ## 配置优先级（高 → 低）
//!
//! 1. **管理端下发** — `admin_config.json` 缓存，`set_var` 强制覆盖
//! 2. **显式环境变量** — shell `export` 或命令行传入
//! 3. **本地 .env** — `desktop-client/.env`（dotenvy 不覆盖已有变量）
//! 4. **客户端默认值** — `ensure_client_defaults()` 兜底
//!
//! ## 启动流程
//!
//! ```text
//! init_logging()
//!   → load_client_env()        // .env 文件 + 客户端隔离默认值
//!   → apply_admin_overrides()  // 管理端配置覆盖（最高优先级）
//!   → Tauri::setup()
//!     → start_ironclaw_engine()  // Config::from_env() → AppBuilder
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ironclaw::channels::web::log_layer::{init_tracing, LogBroadcaster};
use tauri::Manager;

// ── 应用级常量 ────────────────────────────────────────────────────

/// 客户端数据根目录名（位于系统 Application Support 下）。
const APP_DATA_DIR: &str = "ironclaw-desktop";

/// IronClaw 引擎数据子目录（数据库、.env 等）。
const ENGINE_SUBDIR: &str = "ironclaw";

fn main() {
    // LogBroadcaster 在 main 最开始创建，通过 init_tracing 注册 WebLogLayer，
    // 确保引擎启动前的所有日志也能被捕获到内存缓冲区。
    let log_broadcaster = Arc::new(LogBroadcaster::new());
    let _log_level_handle = init_tracing(Arc::clone(&log_broadcaster));

    // ── 配置加载（必须在 Tokio runtime 启动前完成）─────────────
    load_client_env();
    apply_admin_overrides();

    tracing::info!("Starting IronClaw Desktop Client (embedded mode)");

    // ── 启动 Tauri ────────────────────────────────────────────────
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(desktop_client::state::EngineState::new())
        .manage(desktop_client::approval_polling::PendingTicketStore::init())
        // LogBroadcaster 作为 managed state 传给引擎，避免重复创建
        .manage(desktop_client::state::SharedLogBroadcaster(log_broadcaster))
        .setup(|app| {
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    desktop_client::engine::start_ironclaw_engine(app_handle.clone()).await
                {
                    let err_msg = format!("{:#}", e);
                    tracing::error!(error = %err_msg, "IronClaw engine failed to start");

                    // 标记引擎启动失败，让 IPC 命令返回具体错误而非"正在启动中"
                    let engine_state = app_handle.state::<desktop_client::state::EngineState>();
                    engine_state.set_failed(err_msg.clone());

                    let _ = desktop_client::tauri_channel::emit_chat_stream(
                        &app_handle,
                        None,
                        &desktop_client::vercel_ui_protocol::VercelUIStream::Error {
                            error_text: desktop_client::error::friendly_engine_error(&err_msg),
                        },
                    );
                }
            });

            Ok(())
        })
        .invoke_handler(desktop_client::all_tauri_commands!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ═══════════════════════════════════════════════════════════════════
// 配置加载
// ═══════════════════════════════════════════════════════════════════

/// 客户端应用数据根目录。
///
/// macOS: `~/Library/Application Support/ironclaw-desktop/`
/// Linux: `~/.local/share/ironclaw-desktop/`
fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DATA_DIR)
}

/// IronClaw 引擎数据目录（数据库、配置等）。
///
/// 等价于 IronClaw CLI 的 `~/.ironclaw/`，但隔离到客户端专属路径。
fn engine_data_dir() -> PathBuf {
    app_data_dir().join(ENGINE_SUBDIR)
}

/// 加载客户端环境变量。
///
/// 1. 从 `desktop-client/.env` 加载用户配置
/// 2. 设置客户端隔离默认值（`IRONCLAW_BASE_DIR`、`DATABASE_BACKEND` 等）
///
/// dotenvy 不覆盖已有环境变量，所以显式 `export` 的变量优先级更高。
fn load_client_env() {
    // ── 加载 .env 文件 ────────────────────────────────────────────
    load_env_file("desktop-client/.env");

    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        }
        .into()
    });
    load_env_file(&format!("desktop-client/.env.{}", environment));
    env::set_var("ENVIRONMENT", &environment);

    // ── 客户端隔离默认值 ──────────────────────────────────────────
    ensure_client_defaults();
}

/// 加载指定路径的 .env 文件（存在则加载，不存在则跳过）。
fn load_env_file(path: &str) {
    let path = Path::new(path);
    if path.exists() {
        match dotenvy::from_path(path) {
            Ok(_) => tracing::info!("Loaded {}", path.display()),
            Err(e) => tracing::warn!("Failed to load {}: {}", path.display(), e),
        }
    }
}

/// 设置客户端隔离默认值。
///
/// 这些默认值确保 IronClaw 引擎的数据完全隔离在客户端专属目录下，
/// 不会读取或写入全局 `~/.ironclaw/` 目录。
///
/// 只在对应环境变量未设置时生效（`set_if_absent`），
/// 所以 `.env` 文件和显式环境变量可以覆盖这些默认值。
///
/// # 隔离策略
///
/// | 变量 | 默认值 | 作用 |
/// |------|--------|------|
/// | `IRONCLAW_BASE_DIR` | `~/...App Support/ironclaw-desktop/ironclaw/` | 引擎根目录，阻止加载 `~/.ironclaw/.env` |
/// | `DATABASE_BACKEND` | `libsql` | 使用本地 libSQL，跳过 IronClaw 的 `~/.ironclaw/ironclaw.db` 自动检测 |
/// | `LIBSQL_PATH` | `<engine_data_dir>/ironclaw.db` | 数据库文件路径，隔离到客户端目录 |
fn ensure_client_defaults() {
    let engine_dir = engine_data_dir();

    // IRONCLAW_BASE_DIR: 重定向引擎根目录，阻止 load_ironclaw_env() 加载 ~/.ironclaw/.env
    set_if_absent("IRONCLAW_BASE_DIR", &engine_dir.to_string_lossy());

    // DATABASE_BACKEND: 显式设置，跳过 load_ironclaw_env() 中硬编码的
    // ~/.ironclaw/ironclaw.db 自动检测逻辑
    set_if_absent("DATABASE_BACKEND", "libsql");

    // LIBSQL_PATH: 数据库文件存储在客户端专属目录
    let db_path = engine_dir.join("ironclaw.db");
    set_if_absent("LIBSQL_PATH", &db_path.to_string_lossy());

    tracing::debug!(
        ironclaw_base_dir = %engine_dir.display(),
        db_path = %db_path.display(),
        "Client isolation defaults applied"
    );
}

/// 设置环境变量（仅当未设置时）。
fn set_if_absent(key: &str, value: &str) {
    if env::var(key).is_err() {
        env::set_var(key, value);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 管理端配置
// ═══════════════════════════════════════════════════════════════════

/// 管理端配置 JSON key → 环境变量的映射表。
///
/// 后续新增管理端下发的配置项，只需在此表中添加一行。
const ADMIN_CONFIG_MAPPINGS: &[(&str, &str)] = &[
    ("llm_backend", "LLM_BACKEND"),
    ("llm_api_key", "LLM_API_KEY"),
    ("llm_model", "LLM_MODEL"),
    ("llm_base_url", "LLM_BASE_URL"),
    ("skill_registry_url", "CLAWHUB_REGISTRY"),
    ("managed_mode", "MANAGED_MODE"),
];

/// 敏感字段（日志中不打印值）。
const ADMIN_CONFIG_SENSITIVE_KEYS: &[&str] = &["LLM_API_KEY", "ADMIN_API_KEY"];

/// 从本地缓存加载管理端配置并注入环境变量。
///
/// 管理端可下发 LLM API Key、模型名称、安全策略等配置。
/// 使用 `set_var` 强制覆盖，确保管理端配置拥有最高优先级。
///
/// # 配置文件路径
///
/// `~/Library/Application Support/ironclaw-desktop/admin_config.json`
///
/// # 安全
///
/// - API Key 等敏感字段不写入日志
/// - 此函数必须在 Tokio runtime 启动前调用（`set_var` 线程安全要求）
///
/// # 后续改造
///
/// 当前从本地 JSON 缓存读取。后续改为管理端下发时，只需：
/// 1. `admin_sync.rs` 定期从管理端拉取配置并写入缓存文件
/// 2. 此函数无需修改 — 它只负责"缓存 → 环境变量"这一步
fn apply_admin_overrides() {
    let cache_path = app_data_dir().join("admin_config.json");

    if !cache_path.exists() {
        tracing::debug!("No admin config cache at {}", cache_path.display());
        return;
    }

    let content = match std::fs::read_to_string(&cache_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read admin config: {}", e);
            return;
        }
    };

    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to parse admin config: {}", e);
            return;
        }
    };

    let mut injected = 0u32;
    for &(json_key, env_key) in ADMIN_CONFIG_MAPPINGS {
        let val = config.get(json_key).and_then(|v| match v {
            serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        });
        if let Some(val) = val {
            env::set_var(env_key, &val);
            injected += 1;

            if ADMIN_CONFIG_SENSITIVE_KEYS.contains(&env_key) {
                tracing::info!("Admin override: {}=***", env_key);
            } else {
                tracing::info!("Admin override: {}={}", env_key, val);
            }
        }
    }

    if injected > 0 {
        tracing::info!("Applied {} admin config overrides", injected);
    }
}
