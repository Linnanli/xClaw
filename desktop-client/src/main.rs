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
//! 1. **显式环境变量** — shell `export` 或命令行传入
//! 2. **管理端默认模型** — live `/api/client-models`，仅用于启动 LLM provider
//! 3. **管理端策略缓存** — `admin_config.json` 缓存，仅用于非 LLM 启动策略
//! 4. **客户端默认值** — `ensure_client_defaults()` 兜底
//!
//! ## 启动流程
//!
//! ```text
//! init_logging()
//!   → load_client_env()        // 客户端隔离默认值
//!   → apply_admin_overrides()  // 管理端非 LLM 策略覆盖
//!   → Tauri::setup()
//!     → start_ironclaw_engine()  // Config::from_env() → AppBuilder
//! ```

use std::env;
use std::path::PathBuf;
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
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    // Debug-only WebDriver automation plugin: starts an HTTP server on :4445
    // that bridges W3C WebDriver commands into WKWebView. NEVER ship in
    // production (gated by both feature flag AND debug_assertions).
    #[cfg(all(debug_assertions, feature = "webdriver"))]
    let builder = builder.plugin(tauri_plugin_webdriver::init());

    builder
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

/// 加载客户端环境变量默认值。
///
/// 只设置客户端隔离默认值（引擎根目录、`DATABASE_BACKEND` 等）。
/// LLM 配置由显式环境变量或 live `/api/client-models` 提供，客户端启动时不再自动读取
/// `desktop-client/.env*`，避免后端下发配置与本地文件混用。
///
/// 显式 `export` 的变量仍优先于客户端默认值。
fn load_client_env() {
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        }
        .into()
    });
    env::set_var("ENVIRONMENT", &environment);

    // ── 客户端隔离默认值 ──────────────────────────────────────────
    ensure_client_defaults();
}

/// 设置客户端隔离默认值。
///
/// 这些默认值确保 IronClaw 引擎的数据完全隔离在客户端专属目录下，
/// 不会读取或写入全局 `~/.ironclaw/` 目录。
///
/// 只在对应环境变量未设置时生效（`set_if_absent`），
/// 所以显式环境变量可以覆盖这些默认值。
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

/// 管理端缓存 JSON key → 启动环境变量的映射表。
///
/// LLM 配置不再从本地缓存注入，避免旧 `admin_config.json` 在 Admin Backend
/// 已清空 `/api/client-config` 后继续污染启动。Rust 引擎启动链路会从 live
/// `/api/client-models` 拉取默认模型，运行中 legacy client-config 变更仍由
/// `AdminConfigSync` 处理。
const ADMIN_CONFIG_MAPPINGS: &[AdminConfigMapping] = &[
    AdminConfigMapping {
        json_key: "skill_registry_url",
        env_key: "CLAWHUB_REGISTRY",
    },
    AdminConfigMapping {
        json_key: "managed_mode",
        env_key: "MANAGED_MODE",
    },
];

struct AdminConfigMapping {
    json_key: &'static str,
    env_key: &'static str,
}

/// 敏感字段（日志中不打印值）。
const ADMIN_CONFIG_SENSITIVE_KEYS: &[&str] = &[
    "LLM_API_KEY",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "ADMIN_API_KEY",
];

/// 从本地缓存加载管理端非 LLM 策略并注入环境变量。
///
/// LLM API Key、模型名称和 Base URL 不再从缓存注入；这些字段必须来自显式
/// 环境变量、启动期 live `/api/client-models`，或运行中 `AdminConfigSync` 的新配置。
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
/// 此函数只负责"缓存 → 非 LLM 环境变量"这一步。
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

    let current_env: std::collections::HashMap<String, String> = env::vars().collect();
    let resolved = resolve_overrides(&config, &current_env);

    for (env_key, val) in &resolved {
        env::set_var(env_key, val);
        if ADMIN_CONFIG_SENSITIVE_KEYS.contains(&env_key.as_str()) {
            tracing::info!("Admin override: {}=***", env_key);
        } else {
            tracing::info!("Admin override: {}={}", env_key, val);
        }
    }

    if !resolved.is_empty() {
        tracing::info!("Applied {} admin config overrides", resolved.len());
    }
}

/// 纯函数：把 admin 缓存 JSON 解析成启动时允许写入的 `(env_key, value)` 列表。
fn resolve_overrides(
    config: &serde_json::Value,
    _current_env: &std::collections::HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();

    for m in ADMIN_CONFIG_MAPPINGS {
        let Some(val) = extract_admin_value(config, m.json_key) else {
            continue;
        };
        out.push((m.env_key.to_string(), val));
    }

    out
}

fn extract_admin_value(config: &serde_json::Value, key: &str) -> Option<String> {
    config.get(key).and_then(|v| match v {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

#[cfg(test)]
mod apply_admin_overrides_tests {
    use super::{resolve_overrides, ADMIN_CONFIG_MAPPINGS};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn req_cached_startup_mappings_exclude_llm_config() {
        let forbidden = [
            "LLM_BACKEND",
            "LLM_API_KEY",
            "LLM_MODEL",
            "LLM_BASE_URL",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
        ];

        for mapping in ADMIN_CONFIG_MAPPINGS {
            assert!(
                !forbidden.contains(&mapping.env_key),
                "cached startup mapping must not inject LLM config: {}",
                mapping.env_key
            );
        }
    }

    #[test]
    fn req_cached_startup_ignores_llm_fields() {
        let config = json!({
            "llm_backend": "openai",
            "llm_api_key": "sk-from-stale-cache",
            "llm_model": "stale-admin-model",
            "llm_base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "managed_mode": true,
        });
        let resolved = resolve_overrides(&config, &HashMap::new());

        assert_eq!(
            resolved,
            vec![("MANAGED_MODE".to_string(), "true".to_string())]
        );
    }

    #[test]
    fn req_cached_startup_applies_non_llm_policy_fields() {
        let config = json!({
            "managed_mode": true,
            "skill_registry_url": "http://localhost:3000/api/v1?client_token=test",
        });
        let resolved = resolve_overrides(&config, &HashMap::new());

        assert!(resolved
            .iter()
            .any(|(k, v)| k == "MANAGED_MODE" && v == "true"));
        assert!(resolved.iter().any(|(k, v)| {
            k == "CLAWHUB_REGISTRY" && v == "http://localhost:3000/api/v1?client_token=test"
        }));
    }
}
