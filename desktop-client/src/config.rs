use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_name: String,
    pub version: String,
    pub data_dir: String,
    pub auto_lock_timeout_secs: u64,
    pub enable_offline_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "ironclaw-desktop".to_string(),
            version: "0.1.0".to_string(),
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("ironclaw")
                .to_string_lossy()
                .to_string(),
            auto_lock_timeout_secs: 30 * 60, // 30 minutes
            enable_offline_mode: true,
        }
    }
}

impl AppConfig {
    pub fn load(config_path: &Path) -> Result<Self> {
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            let config = toml::from_str(&content)
                .map_err(|e| crate::Error::StorageError(e.to_string()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, config_path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::Error::StorageError(e.to_string()))?;
        std::fs::write(config_path, content)?;
        Ok(())
    }
}
