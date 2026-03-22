//! 会话配置管理
//! 
//! 负责会话超时、令牌刷新等配置的管理

use std::time::Duration;

/// 会话配置
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 会话超时时间（秒）
    pub session_timeout_secs: u64,
    /// 令牌刷新间隔（秒）
    pub token_refresh_interval_secs: u64,
    /// 是否启用自动刷新
    pub auto_refresh_enabled: bool,
}

impl SessionConfig {
    /// 创建默认配置
    pub fn default() -> Self {
        Self {
            session_timeout_secs: 30 * 60, // 30 minutes
            token_refresh_interval_secs: 25 * 60, // 25 minutes (before timeout)
            auto_refresh_enabled: true,
        }
    }
    
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        let session_timeout_secs: u64 = std::env::var("SESSION_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30 * 60);
        
        let token_refresh_interval_secs: u64 = std::env::var("TOKEN_REFRESH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(session_timeout_secs.saturating_sub(5 * 60)); // 5 minutes before timeout
        
        let auto_refresh_enabled: bool = std::env::var("AUTO_REFRESH_ENABLED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        
        Self {
            session_timeout_secs,
            token_refresh_interval_secs,
            auto_refresh_enabled,
        }
    }
    
    /// 从后端配置加载
    /// 
    /// 嵌入式架构下不再读取旧路径 ~/.ironclaw/ironclaw.db，
    /// 直接 fallback 到 from_env()。
    pub fn from_backend() -> Result<Self, String> {
        Ok(Self::from_env())
    }
    
    /// 获取会话超时时长
    pub fn session_timeout(&self) -> Duration {
        Duration::from_secs(self.session_timeout_secs)
    }
    
    /// 获取令牌刷新间隔
    pub fn token_refresh_interval(&self) -> Duration {
        Duration::from_secs(self.token_refresh_interval_secs)
    }
    
    /// 检查是否需要刷新令牌
    pub fn should_refresh_token(&self, last_refresh_secs: u64) -> bool {
        if !self.auto_refresh_enabled {
            return false;
        }
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - last_refresh_secs >= self.token_refresh_interval_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = SessionConfig::default();
        assert_eq!(config.session_timeout_secs, 30 * 60);
        assert_eq!(config.token_refresh_interval_secs, 25 * 60);
        assert!(config.auto_refresh_enabled);
    }
    
    #[test]
    fn test_session_timeout_duration() {
        let config = SessionConfig::default();
        assert_eq!(config.session_timeout(), Duration::from_secs(30 * 60));
    }
    
    #[test]
    fn test_token_refresh_interval_duration() {
        let config = SessionConfig::default();
        assert_eq!(config.token_refresh_interval(), Duration::from_secs(25 * 60));
    }
    
    #[test]
    fn test_should_refresh_token() {
        let config = SessionConfig::default();
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // 刚刚刷新，不需要再次刷新
        assert!(!config.should_refresh_token(now));
        
        // 26分钟前刷新，需要刷新
        assert!(config.should_refresh_token(now - 26 * 60));
        
        // 24分钟前刷新，不需要刷新
        assert!(!config.should_refresh_token(now - 24 * 60));
    }
    
    #[test]
    fn test_auto_refresh_disabled() {
        let mut config = SessionConfig::default();
        config.auto_refresh_enabled = false;
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // 即使超过刷新间隔，也不应该刷新
        assert!(!config.should_refresh_token(now - 26 * 60));
    }
    
    #[test]
    #[serial_test::serial]
    fn test_from_env_with_defaults() {
        // 清除环境变量
        std::env::remove_var("SESSION_TIMEOUT_SECS");
        std::env::remove_var("TOKEN_REFRESH_INTERVAL_SECS");
        std::env::remove_var("AUTO_REFRESH_ENABLED");
        
        let config = SessionConfig::from_env();
        assert_eq!(config.session_timeout_secs, 30 * 60);
        assert_eq!(config.token_refresh_interval_secs, 25 * 60);
        assert!(config.auto_refresh_enabled);
    }
    
    #[test]
    #[serial_test::serial]
    fn test_from_env_with_custom_values() {
        // 设置环境变量
        unsafe {
            std::env::set_var("SESSION_TIMEOUT_SECS", "3600");
            std::env::set_var("TOKEN_REFRESH_INTERVAL_SECS", "3300");
            std::env::set_var("AUTO_REFRESH_ENABLED", "false");
        }
        
        let config = SessionConfig::from_env();
        assert_eq!(config.session_timeout_secs, 3600);
        assert_eq!(config.token_refresh_interval_secs, 3300);
        assert!(!config.auto_refresh_enabled);
        
        // 清除环境变量
        std::env::remove_var("SESSION_TIMEOUT_SECS");
        std::env::remove_var("TOKEN_REFRESH_INTERVAL_SECS");
        std::env::remove_var("AUTO_REFRESH_ENABLED");
    }
}
