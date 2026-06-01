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
/// `requires_companion_keys` 用于切换 `LLM_BACKEND` 时校验依赖 key 是否到位：
/// 若任一候选 env 已存在（含本轮刚写入的），则允许写入；否则跳过并 `warn!`。
/// 这避免 admin 单独推 backend 但客户端 env 缺少对应 API key 导致启动失败。
const ADMIN_CONFIG_MAPPINGS: &[AdminConfigMapping] = &[
    AdminConfigMapping {
        json_key: "llm_api_key",
        env_key: "LLM_API_KEY",
        requires_companion_keys: None,
    },
    AdminConfigMapping {
        json_key: "llm_model",
        env_key: "LLM_MODEL",
        requires_companion_keys: None,
    },
    AdminConfigMapping {
        json_key: "llm_base_url",
        env_key: "LLM_BASE_URL",
        requires_companion_keys: None,
    },
    AdminConfigMapping {
        json_key: "llm_backend",
        env_key: "LLM_BACKEND",
        requires_companion_keys: Some(backend_companion_keys),
    },
    AdminConfigMapping {
        json_key: "skill_registry_url",
        env_key: "CLAWHUB_REGISTRY",
        requires_companion_keys: None,
    },
    AdminConfigMapping {
        json_key: "managed_mode",
        env_key: "MANAGED_MODE",
        requires_companion_keys: None,
    },
];

struct AdminConfigMapping {
    json_key: &'static str,
    env_key: &'static str,
    /// 若 `Some(f)`，写入前调用 `f(value)` 获取候选 companion env key 列表。
    /// 空切片表示该值无需任何 companion key（如 nearai 走 session token）。
    requires_companion_keys: Option<fn(&str) -> &'static [&'static str]>,
}

/// 给定 backend 值，返回可接受的 companion API key env var 候选列表。
///
/// 返回空切片表示该 backend 不依赖任何静态 env key（如 nearai 用 session token）。
/// 列表里只要有一个 key 在 env 中存在，就视为前置条件满足。
///
/// `LLM_API_KEY` 作为通用 fallback 列入 openai/anthropic 候选，
/// 以兼容"admin 同一次推送 backend + llm_api_key"的常见场景。
fn backend_companion_keys(backend: &str) -> &'static [&'static str] {
    match backend {
        "openai" => &["OPENAI_API_KEY", "LLM_API_KEY"],
        "anthropic" => &["ANTHROPIC_API_KEY", "LLM_API_KEY"],
        "openai_compatible" => &["LLM_API_KEY"],
        "nearai" => &[],
        _ => &[],
    }
}

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

/// 纯函数：把 admin 配置 JSON 解析成将要写入的 `(env_key, value)` 列表。
///
/// 两遍处理：
/// 1. 先解析所有不带 `requires_companion_keys` 的字段（API key / model / base_url 等），
///    把它们累加到 `simulated_env`（process env 快照 + 第一遍刚写的值）。
/// 2. 再处理 backend 等带 companion 校验的字段；若候选 env 列表非空但全都缺失，
///    跳过该项并 `tracing::warn!`，避免推下去导致引擎启动 `LlmError::AuthFailed`。
///
/// 抽成独立函数便于测试：调用方传入虚拟 env 即可断言行为。
fn resolve_overrides(
    config: &serde_json::Value,
    current_env: &std::collections::HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut simulated_env: std::collections::HashMap<String, String> = current_env.clone();

    // Pass 1: 写所有非 companion-gated 字段。
    let mut deferred: Vec<(&AdminConfigMapping, String)> = Vec::new();
    for m in ADMIN_CONFIG_MAPPINGS {
        let Some(val) = extract_admin_value(config, m.json_key) else {
            continue;
        };
        if m.requires_companion_keys.is_some() {
            deferred.push((m, val));
        } else {
            simulated_env.insert(m.env_key.to_string(), val.clone());
            out.push((m.env_key.to_string(), val));
        }
    }

    // Pass 2: companion-gated 字段（如 LLM_BACKEND）。
    for (m, val) in deferred {
        if let Some(check) = m.requires_companion_keys {
            let candidates = check(&val);
            if !candidates.is_empty() && !candidates.iter().any(|k| simulated_env.contains_key(*k))
            {
                tracing::warn!(
                    env_key = %m.env_key,
                    value = %val,
                    candidates = ?candidates,
                    "Admin override skipped: switching {} to {} requires one of {:?} in env",
                    m.env_key, val, candidates
                );
                continue;
            }
        }
        simulated_env.insert(m.env_key.to_string(), val.clone());
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

    /// 校验测试和生产代码引用同一份映射表（防漂移）。
    #[test]
    fn test_contract_mappings_include_backend_with_companion_check() {
        let backend = ADMIN_CONFIG_MAPPINGS
            .iter()
            .find(|m| m.env_key == "LLM_BACKEND")
            .expect("LLM_BACKEND mapping must exist");
        assert!(
            backend.requires_companion_keys.is_some(),
            "LLM_BACKEND must be guarded by companion key check"
        );
    }

    #[test]
    fn test_failure_backend_skipped_when_companion_key_missing() {
        let config = json!({ "llm_backend": "openai" });
        let env = HashMap::new();
        let resolved = resolve_overrides(&config, &env);
        assert!(
            !resolved.iter().any(|(k, _)| k == "LLM_BACKEND"),
            "openai backend must be skipped when no OPENAI_API_KEY/LLM_API_KEY in env, got: {:?}",
            resolved
        );
    }

    #[test]
    fn req_admin_override_backend_applied_when_companion_key_present() {
        let config = json!({ "llm_backend": "openai" });
        let mut env = HashMap::new();
        env.insert("OPENAI_API_KEY".to_string(), "sk-pre-set".to_string());
        let resolved = resolve_overrides(&config, &env);
        assert!(
            resolved
                .iter()
                .any(|(k, v)| k == "LLM_BACKEND" && v == "openai"),
            "backend should be applied when OPENAI_API_KEY pre-exists, got: {:?}",
            resolved
        );
    }

    #[test]
    fn req_admin_override_backend_applied_when_admin_pushes_key_too() {
        let config = json!({
            "llm_backend": "openai",
            "llm_api_key": "sk-from-admin",
        });
        let env = HashMap::new(); // 空 env
        let resolved = resolve_overrides(&config, &env);
        // Pass 1 写 LLM_API_KEY，Pass 2 校验 openai 的候选含 LLM_API_KEY → 通过。
        assert!(
            resolved
                .iter()
                .any(|(k, v)| k == "LLM_API_KEY" && v == "sk-from-admin"),
            "LLM_API_KEY should be written in pass 1, got: {:?}",
            resolved
        );
        assert!(
            resolved
                .iter()
                .any(|(k, v)| k == "LLM_BACKEND" && v == "openai"),
            "LLM_BACKEND should pass companion check via just-written LLM_API_KEY, got: {:?}",
            resolved
        );
    }

    #[test]
    fn req_admin_override_openai_compatible_requires_llm_api_key() {
        // 无 LLM_API_KEY 时 openai_compatible 必须跳过。
        let config = json!({ "llm_backend": "openai_compatible" });
        let env = HashMap::new();
        let resolved = resolve_overrides(&config, &env);
        assert!(
            !resolved.iter().any(|(k, _)| k == "LLM_BACKEND"),
            "openai_compatible must be skipped without LLM_API_KEY, got: {:?}",
            resolved
        );

        // 同一次 admin 推送 backend + key → 通过。
        let config_ok = json!({
            "llm_backend": "openai_compatible",
            "llm_api_key": "sk-compat",
        });
        let resolved_ok = resolve_overrides(&config_ok, &env);
        assert!(
            resolved_ok
                .iter()
                .any(|(k, v)| k == "LLM_BACKEND" && v == "openai_compatible"),
            "openai_compatible must pass with admin-pushed llm_api_key, got: {:?}",
            resolved_ok
        );
    }

    #[test]
    fn req_admin_override_nearai_backend_no_key_required() {
        let config = json!({ "llm_backend": "nearai" });
        let env = HashMap::new(); // 空 env
        let resolved = resolve_overrides(&config, &env);
        assert!(
            resolved
                .iter()
                .any(|(k, v)| k == "LLM_BACKEND" && v == "nearai"),
            "nearai backend uses session token, must apply without any API key env, got: {:?}",
            resolved
        );
    }

    #[test]
    fn test_security_audit_companion_check_does_not_leak_key_value() {
        // 此测试确保 resolve_overrides 不返回 env 里的原始 key，
        // 它只读 env 判断 contains_key，不传递 value。
        let config = json!({ "llm_backend": "openai" });
        let mut env = HashMap::new();
        env.insert(
            "OPENAI_API_KEY".to_string(),
            "sk-secret-must-not-leak".into(),
        );
        let resolved = resolve_overrides(&config, &env);
        for (_, v) in &resolved {
            assert!(
                !v.contains("sk-secret-must-not-leak"),
                "resolve_overrides leaked env value: {:?}",
                resolved
            );
        }
    }
}
