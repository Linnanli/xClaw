pub mod auth;
pub mod storage;
pub mod config;
pub mod error;
pub mod commands;
pub mod policy_sync;
pub mod dlp_integration;

pub use error::{Error, Result};
pub use commands::CommandState;

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
