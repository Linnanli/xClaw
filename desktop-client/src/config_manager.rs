//! 配置管理器
//! 
//! 支持从 TOML 文件和环境变量加载配置

use crate::platform_utils;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// 应用配置
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// API 基础 URL
    pub api_base_url: String,
    /// API 超时时间（秒）
    pub api_timeout_secs: u64,
    /// API 重试次数
    pub api_retry_count: u32,
    /// CORS 允许的源
    pub cors_origins: Vec<String>,
    /// 认证方法
    pub auth_method: String,
    /// 日志级别
    pub log_level: String,
    /// 数据库类型
    pub database_type: String,
    /// 数据库路径/URL
    pub database_url: String,
    /// 自定义配置
    pub custom: HashMap<String, String>,
}

impl AppConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        Self {
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            api_timeout_secs: env::var("API_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            api_retry_count: env::var("API_RETRY_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            cors_origins: env::var("CORS_ORIGINS")
                .ok()
                .map(|s| s.split(',').map(|o| o.trim().to_string()).collect())
                .unwrap_or_else(|_| vec![
                    "http://localhost:5173".to_string(),
                    "http://127.0.0.1:5173".to_string(),
                ]),
            auth_method: env::var("AUTH_METHOD")
                .unwrap_or_else(|_| "header".to_string()),
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string()),
            database_type: env::var("DATABASE_TYPE")
                .unwrap_or_else(|_| "sqlite".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| {
                    platform_utils::get_database_path()
                        .to_string_lossy()
                        .to_string()
                }),
            custom: HashMap::new(),
        }
    }
    
    /// 从 TOML 文件加载配置
    pub fn from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        if !platform_utils::file_exists(path) {
            return Err(ConfigError::FileNotFound(path.to_string_lossy().to_string()));
        }
        
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        Self::from_toml(&content)
    }
    
    /// 从 TOML 字符串解析配置
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        // 简单的 TOML 解析（不依赖外部库）
        let mut config = Self::from_env();
        
        for line in content.lines() {
            let line = line.trim();
            
            // 跳过注释和空行
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // 解析 key = value
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();
                
                // 移除引号
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    &value[1..value.len() - 1]
                } else {
                    value
                };
                
                match key {
                    "api_base_url" => config.api_base_url = value.to_string(),
                    "api_timeout_secs" => {
                        if let Ok(timeout) = value.parse() {
                            config.api_timeout_secs = timeout;
                        }
                    }
                    "api_retry_count" => {
                        if let Ok(count) = value.parse() {
                            config.api_retry_count = count;
                        }
                    }
                    "auth_method" => config.auth_method = value.to_string(),
                    "log_level" => config.log_level = value.to_string(),
                    "database_type" => config.database_type = value.to_string(),
                    "database_url" => config.database_url = value.to_string(),
                    _ => {
                        config.custom.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
        
        Ok(config)
    }
    
    /// 保存配置到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // 确保目录存在
        if let Some(parent) = path.parent() {
            platform_utils::create_dir_if_not_exists(parent)
                .map_err(|e| ConfigError::IoError(e.to_string()))?;
        }
        
        let content = self.to_toml();
        fs::write(path, content)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 转换为 TOML 格式
    pub fn to_toml(&self) -> String {
        let mut content = String::new();
        
        content.push_str("# Ironclaw 应用配置\n\n");
        
        content.push_str("[api]\n");
        content.push_str(&format!("base_url = \"{}\"\n", self.api_base_url));
        content.push_str(&format!("timeout_secs = {}\n", self.api_timeout_secs));
        content.push_str(&format!("retry_count = {}\n", self.api_retry_count));
        content.push_str(&format!("auth_method = \"{}\"\n", self.auth_method));
        content.push_str("\n");
        
        content.push_str("[database]\n");
        content.push_str(&format!("type = \"{}\"\n", self.database_type));
        content.push_str(&format!("url = \"{}\"\n", self.database_url));
        content.push_str("\n");
        
        content.push_str("[logging]\n");
        content.push_str(&format!("level = \"{}\"\n", self.log_level));
        content.push_str("\n");
        
        if !self.custom.is_empty() {
            content.push_str("[custom]\n");
            for (key, value) in &self.custom {
                content.push_str(&format!("{} = \"{}\"\n", key, value));
            }
        }
        
        content
    }
    
    /// 验证配置
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.api_base_url.is_empty() {
            return Err(ConfigError::InvalidConfig("api_base_url is empty".to_string()));
        }
        
        if self.api_timeout_secs == 0 {
            return Err(ConfigError::InvalidConfig("api_timeout_secs must be > 0".to_string()));
        }
        
        if !["sqlite", "postgresql", "postgres"].contains(&self.database_type.as_str()) {
            return Err(ConfigError::InvalidConfig(
                format!("unsupported database_type: {}", self.database_type)
            ));
        }
        
        Ok(())
    }
    
    /// 加载或创建默认配置
    pub fn load_or_default() -> Self {
        let config_path = platform_utils::get_config_file_path();
        
        match Self::from_file(&config_path) {
            Ok(config) => config,
            Err(_) => {
                let config = Self::from_env();
                // 尝试保存默认配置
                let _ = config.save(&config_path);
                config
            }
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// 配置错误
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// 文件未找到
    FileNotFound(String),
    /// IO 错误
    IoError(String),
    /// 无效的配置
    InvalidConfig(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "Config file not found: {}", path),
            ConfigError::IoError(e) => write!(f, "IO error: {}", e),
            ConfigError::InvalidConfig(msg) => write!(f, "Invalid config: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_from_env() {
        let config = AppConfig::from_env();
        assert!(!config.api_base_url.is_empty());
        assert!(config.api_timeout_secs > 0);
    }
    
    #[test]
    fn test_from_toml() {
        let toml = r#"
api_base_url = "http://localhost:8000"
api_timeout_secs = 60
api_retry_count = 5
auth_method = "bearer"
log_level = "debug"
database_type = "postgresql"
database_url = "postgresql://localhost/mydb"
        "#;
        
        let config = AppConfig::from_toml(toml).unwrap();
        assert_eq!(config.api_base_url, "http://localhost:8000");
        assert_eq!(config.api_timeout_secs, 60);
        assert_eq!(config.api_retry_count, 5);
        assert_eq!(config.auth_method, "bearer");
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.database_type, "postgresql");
    }
    
    #[test]
    fn test_to_toml() {
        let config = AppConfig::from_env();
        let toml = config.to_toml();
        
        assert!(toml.contains("api_base_url"));
        assert!(toml.contains("database"));
        assert!(toml.contains("logging"));
    }
    
    #[test]
    fn test_validate() {
        let mut config = AppConfig::from_env();
        assert!(config.validate().is_ok());
        
        config.api_timeout_secs = 0;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_custom_config() {
        let toml = r#"
api_base_url = "http://localhost:3000"
custom_key = "custom_value"
        "#;
        
        let config = AppConfig::from_toml(toml).unwrap();
        assert_eq!(config.custom.get("custom_key"), Some(&"custom_value".to_string()));
    }
}
