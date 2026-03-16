//! 网络配置管理
//! 
//! 管理网络相关的配置，包括超时、重试、连接池等

use std::env;
use std::time::Duration;

/// 网络配置
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// 连接超时时间
    pub connect_timeout: Duration,
    /// 请求超时时间
    pub request_timeout: Duration,
    /// 重试次数
    pub max_retries: u32,
    /// 重试延迟（毫秒）
    pub retry_delay_ms: u64,
    /// 指数退避因子
    pub retry_backoff_factor: f64,
    /// 最大连接数
    pub max_connections: usize,
    /// 连接池空闲超时时间
    pub pool_idle_timeout: Duration,
    /// 是否启用 HTTP/2
    pub enable_http2: bool,
    /// 是否启用 gzip 压缩
    pub enable_gzip: bool,
    /// 是否启用 SSL 验证
    pub verify_ssl: bool,
}

impl NetworkConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        Self {
            connect_timeout: Duration::from_secs(
                env::var("NETWORK_CONNECT_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10)
            ),
            request_timeout: Duration::from_secs(
                env::var("NETWORK_REQUEST_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30)
            ),
            max_retries: env::var("NETWORK_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            retry_delay_ms: env::var("NETWORK_RETRY_DELAY_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            retry_backoff_factor: env::var("NETWORK_RETRY_BACKOFF_FACTOR")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0),
            max_connections: env::var("NETWORK_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            pool_idle_timeout: Duration::from_secs(
                env::var("NETWORK_POOL_IDLE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60)
            ),
            enable_http2: env::var("NETWORK_ENABLE_HTTP2")
                .ok()
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(true),
            enable_gzip: env::var("NETWORK_ENABLE_GZIP")
                .ok()
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(true),
            verify_ssl: env::var("NETWORK_VERIFY_SSL")
                .ok()
                .map(|s| s.to_lowercase() == "true")
                .unwrap_or(true),
        }
    }
    
    /// 创建开发环境配置
    pub fn development() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay_ms: 100,
            retry_backoff_factor: 2.0,
            max_connections: 100,
            pool_idle_timeout: Duration::from_secs(60),
            enable_http2: true,
            enable_gzip: true,
            verify_ssl: false,
        }
    }
    
    /// 创建测试环境配置
    pub fn testing() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            max_retries: 1,
            retry_delay_ms: 50,
            retry_backoff_factor: 1.5,
            max_connections: 10,
            pool_idle_timeout: Duration::from_secs(30),
            enable_http2: true,
            enable_gzip: false,
            verify_ssl: false,
        }
    }
    
    /// 创建生产环境配置
    pub fn production() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(10),
            max_retries: 5,
            retry_delay_ms: 200,
            retry_backoff_factor: 2.0,
            max_connections: 500,
            pool_idle_timeout: Duration::from_secs(120),
            enable_http2: true,
            enable_gzip: true,
            verify_ssl: true,
        }
    }
    
    /// 计算重试延迟时间
    /// 
    /// 使用指数退避算法
    pub fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.retry_delay_ms as f64 
            * self.retry_backoff_factor.powi(attempt as i32)) as u64;
        Duration::from_millis(delay_ms)
    }
    
    /// 验证配置
    pub fn validate(&self) -> Result<(), NetworkError> {
        if self.connect_timeout.as_secs() == 0 {
            return Err(NetworkError::InvalidConfig("connect_timeout must be > 0".to_string()));
        }
        
        if self.request_timeout.as_secs() == 0 {
            return Err(NetworkError::InvalidConfig("request_timeout must be > 0".to_string()));
        }
        
        if self.max_connections == 0 {
            return Err(NetworkError::InvalidConfig("max_connections must be > 0".to_string()));
        }
        
        if self.retry_backoff_factor <= 0.0 {
            return Err(NetworkError::InvalidConfig("retry_backoff_factor must be > 0".to_string()));
        }
        
        Ok(())
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// 网络错误
#[derive(Debug, Clone)]
pub enum NetworkError {
    /// 无效的配置
    InvalidConfig(String),
    /// 连接错误
    ConnectionError(String),
    /// 超时错误
    TimeoutError,
    /// 重试失败
    RetryFailed,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::InvalidConfig(msg) => write!(f, "Invalid network config: {}", msg),
            NetworkError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            NetworkError::TimeoutError => write!(f, "Network timeout"),
            NetworkError::RetryFailed => write!(f, "All retries failed"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// 重试策略
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 指数退避因子
    pub backoff_factor: f64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
}

impl RetryPolicy {
    /// 创建新的重试策略
    pub fn new(max_retries: u32, initial_delay_ms: u64, backoff_factor: f64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            backoff_factor,
            max_delay_ms: 30000, // 30 秒
        }
    }
    
    /// 计算重试延迟
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_delay_ms as f64 
            * self.backoff_factor.powi(attempt as i32)) as u64;
        let delay_ms = delay_ms.min(self.max_delay_ms);
        Duration::from_millis(delay_ms)
    }
    
    /// 是否应该重试
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(3, 100, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_from_env() {
        let config = NetworkConfig::from_env();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_development_config() {
        let config = NetworkConfig::development();
        assert_eq!(config.connect_timeout.as_secs(), 10);
        assert_eq!(config.request_timeout.as_secs(), 30);
        assert_eq!(config.max_retries, 3);
        assert!(!config.verify_ssl);
    }
    
    #[test]
    fn test_testing_config() {
        let config = NetworkConfig::testing();
        assert_eq!(config.connect_timeout.as_secs(), 5);
        assert_eq!(config.request_timeout.as_secs(), 10);
        assert_eq!(config.max_retries, 1);
    }
    
    #[test]
    fn test_production_config() {
        let config = NetworkConfig::production();
        assert_eq!(config.connect_timeout.as_secs(), 5);
        assert_eq!(config.request_timeout.as_secs(), 10);
        assert_eq!(config.max_retries, 5);
        assert!(config.verify_ssl);
    }
    
    #[test]
    fn test_calculate_retry_delay() {
        let config = NetworkConfig::development();
        
        let delay0 = config.calculate_retry_delay(0);
        let delay1 = config.calculate_retry_delay(1);
        let delay2 = config.calculate_retry_delay(2);
        
        assert!(delay1 > delay0);
        assert!(delay2 > delay1);
    }
    
    #[test]
    fn test_validate() {
        let config = NetworkConfig::development();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::new(3, 100, 2.0);
        
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        
        let delay0 = policy.calculate_delay(0);
        let delay1 = policy.calculate_delay(1);
        assert!(delay1 > delay0);
    }
}
