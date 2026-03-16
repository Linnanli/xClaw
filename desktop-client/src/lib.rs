pub mod auth;
pub mod storage;
pub mod config;
pub mod error;
pub mod commands;
pub mod policy_sync;
pub mod dlp_integration;
pub mod offline_mode;
pub mod extension_manager;
pub mod routine_manager;
pub mod skill_manager;
pub mod api_client;
pub mod sse_client;
pub mod environment_checker;

pub use error::{Error, Result};
pub use commands::CommandState;
pub use sse_client::{SseClient, SseEvent};
pub use environment_checker::{EnvironmentChecker, EnvironmentConfig, Environment};

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
