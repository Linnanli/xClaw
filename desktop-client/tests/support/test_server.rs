//! TestServer 组件 - 用于集成测试的 Web Gateway 服务器
//!
//! 提供启动和管理测试用 Web Gateway 实例的功能，支持：
//! - 动态端口分配（避免端口冲突）
//! - RAII 模式自动清理
//! - 配置好的 ApiClient 创建

use std::net::SocketAddr;

use tokio::sync::{mpsc, oneshot};

use desktop_client::api_client::ApiClient;

/// 测试服务器错误类型
#[derive(Debug, thiserror::Error)]
pub enum TestServerError {
    #[error("Server startup failed: {0}")]
    StartupFailed(String),
    
    #[error("Server shutdown failed: {0}")]
    ShutdownFailed(String),
}

/// TestServer - 管理测试用的 Web Gateway 实例
///
/// 使用 RAII 模式，当 TestServer 被 drop 时自动关闭服务器。
///
/// # 示例
///
/// ```no_run
/// use desktop_client::tests::support::TestServer;
///
/// #[tokio::test]
/// async fn test_api_call() {
///     let server = TestServer::start().await.unwrap();
///     let client = server.create_client();
///     
///     // 使用 client 进行 API 调用...
///     
///     // server 在作用域结束时自动关闭
/// }
/// ```
pub struct TestServer {
    /// 服务器绑定的地址
    addr: SocketAddr,
    
    /// 用于关闭服务器的通道
    shutdown_tx: Option<oneshot::Sender<()>>,
    
    /// 认证令牌
    auth_token: String,
}

impl TestServer {
    /// 创建并启动测试服务器
    ///
    /// 服务器将绑定到 `127.0.0.1:0`（动态端口分配），
    /// 使用内存数据库进行数据隔离。
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回 `TestServerError::StartupFailed`。
    pub async fn start() -> Result<Self, TestServerError> {
        Self::start_with_token("test-token-123").await
    }
    
    /// 使用指定的认证令牌创建并启动测试服务器
    ///
    /// # 参数
    ///
    /// * `auth_token` - 用于 API 认证的令牌
    ///
    /// # 错误
    ///
    /// 如果服务器启动失败，返回 `TestServerError::StartupFailed`。
    pub async fn start_with_token(auth_token: &str) -> Result<Self, TestServerError> {
        use std::sync::Arc;
        use ironclaw::agent::SessionManager;
        use ironclaw::db::libsql::LibSqlBackend;
        use ironclaw::db::Database;
        
        // 创建消息通道（用于 GatewayState）
        let (msg_tx, mut _msg_rx) = mpsc::channel(100);
        
        // 创建内存数据库
        let db = LibSqlBackend::new_memory()
            .await
            .map_err(|e| TestServerError::StartupFailed(format!("Failed to create database: {}", e)))?;
        
        // 运行数据库迁移
        db.run_migrations()
            .await
            .map_err(|e| TestServerError::StartupFailed(format!("Failed to run migrations: {}", e)))?;
        
        let db: Arc<dyn Database> = Arc::new(db);
        
        // 创建 SessionManager
        let session_manager = Arc::new(SessionManager::new());
        
        // 重新创建 GatewayState，包含所有必要的字段
        let state = Arc::new(ironclaw::channels::web::server::GatewayState {
            msg_tx: tokio::sync::RwLock::new(Some(msg_tx)),
            sse: ironclaw::channels::web::sse::SseManager::new(),
            workspace: None,
            session_manager: Some(session_manager),
            log_broadcaster: None,
            log_level_handle: None,
            extension_manager: None,
            tool_registry: None,
            store: Some(db),
            job_manager: None,
            prompt_queue: None,
            user_id: "test-user".to_string(),
            shutdown_tx: tokio::sync::RwLock::new(None),
            ws_tracker: Some(Arc::new(ironclaw::channels::web::ws::WsConnectionTracker::new())),
            llm_provider: None,
            skill_registry: None,
            skill_catalog: None,
            scheduler: None,
            chat_rate_limiter: ironclaw::channels::web::server::RateLimiter::new(30, 60),
            oauth_rate_limiter: ironclaw::channels::web::server::RateLimiter::new(10, 60),
            registry_entries: Vec::new(),
            cost_guard: None,
            routine_engine: Arc::new(tokio::sync::RwLock::new(None)),
            startup_time: std::time::Instant::now(),
        });
        
        // 绑定到动态端口
        let addr: SocketAddr = "127.0.0.1:0"
            .parse()
            .expect("hard-coded address must parse");
        
        // 启动服务器
        let bound_addr = ironclaw::channels::web::server::start_server(
            addr,
            state.clone(),
            auth_token.to_string(),
        )
        .await
        .map_err(|e| TestServerError::StartupFailed(e.to_string()))?;
        
        // 获取 shutdown_tx（用于后续关闭服务器）
        let shutdown_tx = state.shutdown_tx.write().await.take();
        
        // 保持 _msg_rx 打开，以防止通道关闭
        // 注意：这里我们需要在后台任务中保持接收端打开
        tokio::spawn(async move {
            let _ = _msg_rx.recv().await;
        });
        
        Ok(Self {
            addr: bound_addr,
            shutdown_tx,
            auth_token: auth_token.to_string(),
        })
    }
    
    /// 获取服务器绑定的地址
    ///
    /// # 返回
    ///
    /// 服务器的 `SocketAddr`，包含实际绑定的 IP 和端口。
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
    
    /// 获取服务器的基础 URL
    ///
    /// # 返回
    ///
    /// 格式为 `http://127.0.0.1:{port}` 的字符串。
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
    
    /// 创建配置好的 ApiClient
    ///
    /// 返回的 ApiClient 已经配置了正确的 base_url 和认证令牌。
    ///
    /// # 返回
    ///
    /// 配置好的 `ApiClient` 实例。
    pub fn create_client(&self) -> ApiClient {
        ApiClient::new_with_token(self.base_url(), self.auth_token.clone())
    }
    
    /// 使用指定的认证令牌创建 ApiClient
    ///
    /// # 参数
    ///
    /// * `token` - 自定义的认证令牌
    ///
    /// # 返回
    ///
    /// 配置好的 `ApiClient` 实例。
    pub fn create_client_with_token(&self, token: &str) -> ApiClient {
        ApiClient::new_with_token(self.base_url(), token.to_string())
    }
    
    /// 获取认证令牌
    ///
    /// # 返回
    ///
    /// 服务器使用的认证令牌字符串。
    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }
    
    /// 手动关闭服务器
    ///
    /// 通常不需要手动调用，因为 Drop trait 会自动关闭服务器。
    ///
    /// # 错误
    ///
    /// 如果服务器已经关闭或关闭失败，返回 `TestServerError::ShutdownFailed`。
    pub async fn shutdown(mut self) -> Result<(), TestServerError> {
        if let Some(tx) = self.shutdown_tx.take() {
            tx.send(())
                .map_err(|_| TestServerError::ShutdownFailed("Failed to send shutdown signal".to_string()))?;
            
            // 等待一小段时间让服务器完成关闭
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            Ok(())
        } else {
            Err(TestServerError::ShutdownFailed("Server already shut down".to_string()))
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // 尝试发送关闭信号
        // 注意：这里不能使用 async，所以只能尽力而为
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
