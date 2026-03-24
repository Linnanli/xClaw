pub mod auth;
pub mod storage;
pub mod config;
pub mod error;
pub mod commands;
pub mod policy_sync;
pub mod enterprise_policy_sync;
pub mod dlp;
pub mod dlp_integration;
pub mod offline_mode;
pub mod extension_manager;
pub mod routine_manager;
pub mod skill_manager;
pub mod api_client;
pub mod sse_client;
pub mod environment_checker;
pub mod platform_utils;
pub mod auth_token_manager;
pub mod database_config;
pub mod config_manager;
pub mod network_config;
pub mod memory_manager;
pub mod session_config;
pub mod token_refresh_service;
pub mod embedded_server;

// ── IronClaw 嵌入模块 ──────────────────────────────────────────────
pub mod tauri_channel;
pub mod state;
pub mod engine;
pub mod ipc;
pub mod admin_sync;
pub mod data_reporter;
pub mod safety_bridge;

// 测试模块（仅在测试时编译）
#[cfg(test)]
#[path = "tauri_channel_tests.rs"]
mod tauri_channel_tests;

#[cfg(test)]
#[path = "admin_sync_tests.rs"]
mod admin_sync_tests;

#[cfg(test)]
#[path = "data_reporter_tests.rs"]
mod data_reporter_tests;

#[cfg(test)]
#[path = "safety_bridge_tests.rs"]
mod safety_bridge_tests;

#[cfg(test)]
#[path = "integration_tests.rs"]
mod integration_tests;

#[cfg(test)]
#[path = "engine_startup_tests.rs"]
mod engine_startup_tests;

#[cfg(test)]
mod tests {
    pub mod dlp_policy_sync_integration_tests {
        include!("dlp/policy_sync_integration_tests.rs");
    }
}

pub use error::{Error, Result};
pub use commands::{CommandState, SseSubscriptionManager};
pub use sse_client::{SseClient, SseEvent};
pub use environment_checker::{EnvironmentChecker, EnvironmentConfig, Environment};
pub use platform_utils::{get_app_data_dir, get_config_dir, get_cache_dir, get_os, get_os_name};
pub use auth_token_manager::{AuthTokenManager, TokenError, is_valid_token, clean_token};
pub use database_config::{DatabaseBackend, DatabaseError};
pub use config_manager::{AppConfig, ConfigError};
pub use network_config::{NetworkConfig, NetworkError, RetryPolicy};

/// 所有注册到 Tauri invoke_handler 的命令列表。
///
/// 用 macro_rules! 封装，确保 `main.rs` 和测试使用**同一份**注册表。
/// 新增命令时只需在此处添加一行，main.rs 和契约测试自动同步。
///
/// # 契约测试
///
/// `tests/tauri_command_contract_tests.rs` 使用此宏验证：
/// 1. 所有命令都能被 Tauri IPC 路由（不会出现 "Command not found"）
/// 2. 前端 `invoke('xxx')` 调用的命令名与注册表一致
#[macro_export]
macro_rules! all_tauri_commands {
    () => {
        tauri::generate_handler![
            // ── 聊天 ────────────────────────────────────────────
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
            desktop_client::ipc::ic_extension_setup,
            desktop_client::ipc::ic_extension_setup_submit,
            // ── 任务管理 ────────────────────────────────────────
            desktop_client::ipc::ic_job_events,
            desktop_client::ipc::ic_job_prompt,
            // ── 日程管理 ────────────────────────────────────────
            desktop_client::ipc::ic_routine_runs,
            // ── 工具审批 ────────────────────────────────────────
            desktop_client::ipc::ic_approve_tool,
            desktop_client::ipc::ic_deny_tool,
            // ── DLP 桥接 ────────────────────────────────────────
            desktop_client::ipc::scan_user_input,
            desktop_client::ipc::scan_outbound_request,
            desktop_client::ipc::sanitize_for_storage,
            desktop_client::ipc::check_http_request,
            desktop_client::ipc::get_dlp_config,
            desktop_client::ipc::update_dlp_config,
            desktop_client::ipc::get_dlp_statistics,
            desktop_client::ipc::sync_dlp_rules_from_admin,
            // ── 模型配置 ─────────────────────────────────────────
            desktop_client::ipc::get_available_models,
            desktop_client::ipc::get_custom_models,
            desktop_client::ipc::create_custom_model,
            desktop_client::ipc::update_custom_model,
            desktop_client::ipc::delete_custom_model,
            desktop_client::ipc::test_model_connection,
            // ── 认证 ────────────────────────────────────────────
            desktop_client::commands::get_auth_token,
        ]
    };
}

#[derive(Debug, Clone)]
pub struct DesktopClientConfig {
    pub app_name: String,
    pub data_dir: std::path::PathBuf,
}

impl Default for DesktopClientConfig {
    fn default() -> Self {
        Self {
            app_name: "ironclaw-desktop".to_string(),
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("ironclaw"),
        }
    }
}
