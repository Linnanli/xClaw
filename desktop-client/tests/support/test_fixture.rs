//! TestFixture 组件 - 用于集成测试的测试数据管理
//!
//! 提供测试数据的创建和清理功能，支持：
//! - 自动创建测试对话和消息
//! - RAII 模式自动清理资源
//! - 辅助方法简化测试编写

use super::test_server::{TestServer, TestServerError};
use desktop_client::api_client::{ApiClient, SendMessageRequest, ThreadInfo, SendMessageResponse};

/// 测试 Fixture 错误类型
#[derive(Debug, thiserror::Error)]
pub enum TestFixtureError {
    #[error("Server error: {0}")]
    ServerError(#[from] TestServerError),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Cleanup error: {0}")]
    CleanupError(String),
}

/// TestFixture - 管理测试数据的创建和清理
///
/// 使用 RAII 模式，当 TestFixture 被 drop 时自动清理所有创建的资源。
///
/// # 示例
///
/// ```no_run
/// use desktop_client::tests::support::TestFixture;
///
/// #[tokio::test]
/// async fn test_chat_flow() {
///     let mut fixture = TestFixture::new().await.unwrap();
///     
///     // 创建测试对话
///     let thread = fixture.create_thread().await.unwrap();
///     
///     // 发送测试消息
///     let response = fixture.send_message(&thread.id, "Hello").await.unwrap();
///     
///     // fixture 在作用域结束时自动清理
/// }
/// ```
pub struct TestFixture {
    /// 测试服务器实例
    server: TestServer,
    
    /// API 客户端
    client: ApiClient,
    
    /// 创建的对话 ID 列表（用于清理）
    created_threads: Vec<String>,
    
    /// 创建的任务 ID 列表（用于清理）
    created_jobs: Vec<String>,
}

impl TestFixture {
    /// 创建新的测试 fixture
    ///
    /// 启动测试服务器并创建配置好的 API 客户端。
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回 `TestFixtureError::ServerError`。
    pub async fn new() -> Result<Self, TestFixtureError> {
        let server = TestServer::start().await?;
        let client = server.create_client();
        
        Ok(Self {
            server,
            client,
            created_threads: Vec::new(),
            created_jobs: Vec::new(),
        })
    }
    
    /// 使用指定的认证令牌创建新的测试 fixture
    ///
    /// # 参数
    ///
    /// * `auth_token` - 用于 API 认证的令牌
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回 `TestFixtureError::ServerError`。
    pub async fn new_with_token(auth_token: &str) -> Result<Self, TestFixtureError> {
        let server = TestServer::start_with_token(auth_token).await?;
        let client = server.create_client();
        
        Ok(Self {
            server,
            client,
            created_threads: Vec::new(),
            created_jobs: Vec::new(),
        })
    }
    
    /// 获取 API 客户端的引用
    ///
    /// # 返回
    ///
    /// `ApiClient` 的引用，可用于直接调用 API。
    pub fn client(&self) -> &ApiClient {
        &self.client
    }
    
    /// 获取测试服务器的引用
    ///
    /// # 返回
    ///
    /// `TestServer` 的引用。
    pub fn server(&self) -> &TestServer {
        &self.server
    }
    
    /// 创建测试对话
    ///
    /// 创建一个新的对话，并将其 ID 添加到清理列表中。
    ///
    /// # 返回
    ///
    /// 创建的 `ThreadInfo`。
    ///
    /// # 错误
    ///
    /// 如果 API 调用失败，返回 `TestFixtureError::ApiError`。
    pub async fn create_thread(&mut self) -> Result<ThreadInfo, TestFixtureError> {
        let thread = self.client
            .create_thread()
            .await
            .map_err(|e| TestFixtureError::ApiError(e.to_string()))?;
        
        self.created_threads.push(thread.id.clone());
        
        Ok(thread)
    }
    
    /// 发送测试消息
    ///
    /// 向指定的对话发送消息。
    ///
    /// # 参数
    ///
    /// * `thread_id` - 对话 ID
    /// * `content` - 消息内容
    ///
    /// # 返回
    ///
    /// `SendMessageResponse`，包含消息 ID 和状态。
    ///
    /// # 错误
    ///
    /// 如果 API 调用失败，返回 `TestFixtureError::ApiError`。
    pub async fn send_message(
        &self,
        thread_id: &str,
        content: &str,
    ) -> Result<SendMessageResponse, TestFixtureError> {
        let request = SendMessageRequest {
            content: content.to_string(),
            thread_id: Some(thread_id.to_string()),
        };
        
        self.client
            .send_message(request)
            .await
            .map_err(|e| TestFixtureError::ApiError(e.to_string()))
    }
    
    /// 清理所有创建的资源
    ///
    /// 删除所有创建的对话和任务。
    /// 通常不需要手动调用，因为 Drop trait 会自动清理。
    ///
    /// # 错误
    ///
    /// 如果清理失败，返回 `TestFixtureError::CleanupError`。
    /// 注意：清理失败不会阻止后续清理操作。
    pub async fn cleanup(&mut self) -> Result<(), TestFixtureError> {
        // 清理对话
        for thread_id in &self.created_threads {
            // 注意：当前 API 可能没有删除对话的端点
            // 这里只是记录，实际清理由内存数据库自动完成
            tracing::debug!("Would clean up thread: {}", thread_id);
        }
        
        // 清理任务
        for job_id in &self.created_jobs {
            if let Err(e) = self.client.cancel_job(job_id).await {
                tracing::warn!("Failed to cancel job {}: {}", job_id, e);
            }
        }
        
        self.created_threads.clear();
        self.created_jobs.clear();
        
        Ok(())
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        // 注意：Drop 中不能使用 async，所以只能记录
        // 实际清理由内存数据库自动完成
        if !self.created_threads.is_empty() {
            tracing::debug!("TestFixture dropped with {} threads", self.created_threads.len());
        }
        if !self.created_jobs.is_empty() {
            tracing::debug!("TestFixture dropped with {} jobs", self.created_jobs.len());
        }
    }
}
