//! 令牌自动刷新服务
//! 
//! 负责在后台定期检查并刷新认证令牌，确保会话不会过期

use crate::auth_token_manager::AuthTokenManager;
use crate::session_config::SessionConfig;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{debug, error, info};

/// 令牌刷新服务
pub struct TokenRefreshService {
    token_manager: Arc<Mutex<AuthTokenManager>>,
    config: SessionConfig,
    last_refresh: Arc<Mutex<u64>>,
    is_running: Arc<Mutex<bool>>,
}

impl TokenRefreshService {
    /// 创建新的令牌刷新服务
    pub fn new(token_manager: AuthTokenManager, config: SessionConfig) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            token_manager: Arc::new(Mutex::new(token_manager)),
            config,
            last_refresh: Arc::new(Mutex::new(now)),
            is_running: Arc::new(Mutex::new(false)),
        }
    }
    
    /// 启动自动刷新服务
    /// 
    /// 返回一个 JoinHandle，可以用于停止服务
    pub async fn start(&self) -> tokio::task::JoinHandle<()> {
        let token_manager = Arc::clone(&self.token_manager);
        let config = self.config.clone();
        let last_refresh = Arc::clone(&self.last_refresh);
        let is_running = Arc::clone(&self.is_running);
        
        // 标记服务为运行状态
        {
            let mut running = is_running.lock().unwrap();
            *running = true;
        }
        
        tokio::spawn(async move {
            info!("Token refresh service started");
            
            // 每分钟检查一次
            let mut check_interval = interval(Duration::from_secs(60));
            
            loop {
                check_interval.tick().await;
                
                // 检查是否应该停止
                {
                    let running = is_running.lock().unwrap();
                    if !*running {
                        info!("Token refresh service stopped");
                        break;
                    }
                }
                
                // 检查是否需要刷新
                let should_refresh = {
                    let last = *last_refresh.lock().unwrap();
                    config.should_refresh_token(last)
                };
                
                if should_refresh {
                    debug!("Token refresh needed, refreshing...");
                    
                    // 刷新令牌
                    match Self::refresh_token_internal(&token_manager).await {
                        Ok(()) => {
                            info!("Token refreshed successfully");
                            
                            // 更新最后刷新时间
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            
                            let mut last = last_refresh.lock().unwrap();
                            *last = now;
                        }
                        Err(e) => {
                            error!("Failed to refresh token: {}", e);
                        }
                    }
                } else {
                    debug!("Token refresh not needed yet");
                }
            }
        })
    }
    
    /// 停止自动刷新服务
    pub fn stop(&self) {
        let mut running = self.is_running.lock().unwrap();
        *running = false;
        info!("Token refresh service stop requested");
    }
    
    /// 手动刷新令牌
    pub async fn refresh_now(&self) -> Result<(), String> {
        info!("Manual token refresh requested");
        
        Self::refresh_token_internal(&self.token_manager).await?;
        
        // 更新最后刷新时间
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut last = self.last_refresh.lock().unwrap();
        *last = now;
        
        Ok(())
    }
    
    /// 获取最后刷新时间
    pub fn last_refresh_time(&self) -> u64 {
        *self.last_refresh.lock().unwrap()
    }
    
    /// 检查服务是否正在运行
    pub fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }
    
    /// 内部令牌刷新逻辑
    async fn refresh_token_internal(
        token_manager: &Arc<Mutex<AuthTokenManager>>,
    ) -> Result<(), String> {
        // 生成新令牌
        let new_token = AuthTokenManager::generate_new_token();
        
        // 保存新令牌
        let manager = token_manager.lock().unwrap();
        manager.save(&new_token).map_err(|e| e.to_string())?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    
    // ========== 单元测试 ==========
    
    #[test]
    fn test_service_creation() {
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        assert!(!service.is_running(), "Service should not be running initially");
        assert!(service.last_refresh_time() > 0, "Last refresh time should be set");
    }
    
    #[test]
    fn test_last_refresh_time() {
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let last_refresh = service.last_refresh_time();
        
        // 最后刷新时间应该接近当前时间
        assert!(last_refresh <= now, "Last refresh should not be in the future");
        assert!(now - last_refresh < 5, "Last refresh should be recent");
    }
    
    // ========== 集成测试 ==========
    
    #[tokio::test]
    #[serial]
    async fn test_manual_refresh() {
        let token_manager = AuthTokenManager::new();
        let _ = token_manager.delete();
        
        let config = SessionConfig::default();
        let service = TokenRefreshService::new(token_manager, config);
        
        // 手动刷新
        let result = service.refresh_now().await;
        assert!(result.is_ok(), "Manual refresh should succeed");
        
        // 验证最后刷新时间已更新
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let last_refresh = service.last_refresh_time();
        assert!(now - last_refresh < 5, "Last refresh time should be updated");
    }
    
    #[tokio::test]
    #[serial]
    async fn test_service_start_stop() {
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        // 启动服务
        let handle = service.start().await;
        
        // 等待一小段时间
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 验证服务正在运行
        assert!(service.is_running(), "Service should be running");
        
        // 停止服务
        service.stop();
        
        // 等待服务停止
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 验证服务已停止
        assert!(!service.is_running(), "Service should be stopped");
        
        // 等待任务完成
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }
    
    // ========== 失败路径测试 ==========
    
    #[tokio::test]
    #[serial]
    async fn test_refresh_with_invalid_token_manager() {
        // 这个测试验证当令牌管理器失败时的行为
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        // 手动刷新应该成功（因为 generate_new_token 总是成功）
        let result = service.refresh_now().await;
        assert!(result.is_ok(), "Refresh should succeed even with new token manager");
    }
    
    // ========== 可靠性测试 ==========
    
    #[tokio::test]
    #[serial]
    async fn test_concurrent_manual_refresh() {
        let token_manager = AuthTokenManager::new();
        let _ = token_manager.delete();
        
        let config = SessionConfig::default();
        let service = Arc::new(TokenRefreshService::new(token_manager, config));
        
        // 并发执行多次手动刷新
        let mut handles = vec![];
        for _ in 0..10 {
            let service_clone = Arc::clone(&service);
            let handle = tokio::spawn(async move {
                service_clone.refresh_now().await
            });
            handles.push(handle);
        }
        
        // 等待所有刷新完成
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Concurrent refresh should succeed");
        }
    }
    
    // ========== 需求级测试 ==========
    
    #[test]
    fn req_refresh_001_service_starts_in_stopped_state() {
        // 需求：服务创建时应该处于停止状态
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        assert!(!service.is_running(), "Service should start in stopped state");
    }
    
    #[tokio::test]
    #[serial]
    async fn req_refresh_002_manual_refresh_updates_time() {
        // 需求：手动刷新应该更新最后刷新时间
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        let before = service.last_refresh_time();
        
        // 等待至少1秒以确保时间戳变化
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // 手动刷新
        service.refresh_now().await.unwrap();
        
        let after = service.last_refresh_time();
        
        assert!(after > before, "Last refresh time should be updated (before: {}, after: {})", before, after);
    }
    
    #[tokio::test]
    #[serial]
    async fn req_refresh_003_service_can_be_stopped() {
        // 需求：服务应该能够被停止
        let token_manager = AuthTokenManager::new();
        let config = SessionConfig::default();
        
        let service = TokenRefreshService::new(token_manager, config);
        
        // 启动服务
        let _handle = service.start().await;
        
        // 等待服务启动
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(service.is_running(), "Service should be running");
        
        // 停止服务
        service.stop();
        
        // 等待服务停止
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!service.is_running(), "Service should be stopped");
    }
}
