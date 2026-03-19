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

// 测试模块（仅在测试时编译）
#[cfg(test)]
mod tests {
    pub mod dlp_policy_sync_integration_tests {
        include!("dlp/policy_sync_integration_tests.rs");
    }
}

pub use error::{Error, Result};
pub use commands::CommandState;
pub use sse_client::{SseClient, SseEvent};
pub use environment_checker::{EnvironmentChecker, EnvironmentConfig, Environment};
pub use platform_utils::{get_app_data_dir, get_config_dir, get_cache_dir, get_os, get_os_name};
pub use auth_token_manager::{AuthTokenManager, TokenError, is_valid_token, clean_token};
pub use database_config::{DatabaseBackend, DatabaseError};
pub use config_manager::{AppConfig, ConfigError};
pub use network_config::{NetworkConfig, NetworkError, RetryPolicy};

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
