//! 测试支持模块
//!
//! 提供集成测试所需的辅助工具和类型。

pub mod test_server;
pub mod test_fixture;
pub mod generators;

pub use test_server::TestServer;
pub use test_fixture::TestFixture;
pub use generators::*;

// 从 api_client 导入数据类型
pub use desktop_client::api_client::ApprovalRequest;

/// 测试配置
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// 测试服务器端口（0 表示动态分配）
    pub port: u16,
    
    /// 认证令牌
    pub auth_token: String,
    
    /// 数据库路径（:memory: 表示内存数据库）
    pub db_path: String,
    
    /// 请求超时时间
    pub timeout: std::time::Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            port: 0, // 动态分配
            auth_token: "test-token-123".to_string(),
            db_path: ":memory:".to_string(),
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

/// 测试错误类型
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Server startup failed: {0}")]
    ServerStartup(String),
    
    #[error("API call failed: {0}")]
    ApiCall(String),
    
    #[error("Assertion failed: {0}")]
    Assertion(String),
    
    #[error("Cleanup failed: {0}")]
    Cleanup(String),
}
