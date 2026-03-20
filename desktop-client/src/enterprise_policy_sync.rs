//! 企业级策略同步管理器
//!
//! 提供集中化策略管理、实时同步、审计追踪等企业级功能

use crate::dlp::DlpResult;
use crate::policy_sync::{PolicySyncManager, DlpPolicy, SensitiveOpPolicy, PolicyVersion};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{interval, Interval};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// 远程策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePolicyConfig {
    /// 策略服务器URL
    pub server_url: String,
    /// 同步间隔
    pub sync_interval: Duration,
    /// 认证令牌
    pub auth_token: String,
    /// 是否启用实时推送
    pub enable_realtime: bool,
    /// 连接超时时间
    pub connection_timeout: Duration,
    /// 重试次数
    pub max_retries: u32,
}

impl Default for RemotePolicyConfig {
    fn default() -> Self {
        Self {
            server_url: "https://policy.example.com".to_string(),
            sync_interval: Duration::from_secs(300), // 5分钟
            auth_token: String::new(),
            enable_realtime: true,
            connection_timeout: Duration::from_secs(30),
            max_retries: 3,
        }
    }
}

/// 策略同步状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicySyncStatus {
    /// 未初始化
    NotInitialized,
    /// 同步中
    Syncing,
    /// 同步成功
    Success,
    /// 同步失败
    Failed(String),
    /// 连接断开
    Disconnected,
}

/// 策略变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyChangeEvent {
    /// 事件ID
    pub id: Uuid,
    /// 事件类型
    pub event_type: PolicyChangeType,
    /// 策略ID
    pub policy_id: String,
    /// 变更内容
    pub changes: serde_json::Value,
    /// 时间戳
    pub timestamp: u64,
    /// 操作者
    pub operator: String,
}

/// 策略变更类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyChangeType {
    /// 创建策略
    Created,
    /// 更新策略
    Updated,
    /// 删除策略
    Deleted,
    /// 启用策略
    Enabled,
    /// 禁用策略
    Disabled,
}

/// 远程策略响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct RemotePoliciesResponse {
    dlp_policies: Vec<DlpPolicy>,
    sensitive_ops_policies: Vec<SensitiveOpPolicy>,
    version: PolicyVersion,
}

/// 策略同步统计
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PolicySyncStats {
    /// 总同步次数
    pub total_syncs: u64,
    /// 成功同步次数
    pub successful_syncs: u64,
    /// 失败同步次数
    pub failed_syncs: u64,
    /// 最后同步时间
    pub last_sync_time: Option<u64>,
    /// 最后成功同步时间
    pub last_successful_sync: Option<u64>,
    /// 策略总数
    pub total_policies: usize,
    /// DLP策略数量
    pub dlp_policies_count: usize,
    /// 敏感操作策略数量
    pub sensitive_ops_policies_count: usize,
}

/// 企业级策略同步管理器
pub struct EnterprisePolicySyncManager {
    /// 本地策略管理器
    local_manager: RwLock<PolicySyncManager>,
    /// 远程配置
    remote_config: RwLock<RemotePolicyConfig>,
    /// 同步状态
    sync_status: RwLock<PolicySyncStatus>,
    /// 同步统计
    sync_stats: RwLock<PolicySyncStats>,
    /// 策略变更事件历史
    change_events: RwLock<Vec<PolicyChangeEvent>>,
    /// 同步定时器
    sync_timer: RwLock<Option<Interval>>,
}

impl EnterprisePolicySyncManager {
    /// 创建新的企业级策略同步管理器
    #[instrument]
    pub fn new(remote_config: RemotePolicyConfig) -> Self {
        info!("Initializing enterprise policy sync manager");
        
        Self {
            local_manager: RwLock::new(PolicySyncManager::new()),
            remote_config: RwLock::new(remote_config),
            sync_status: RwLock::new(PolicySyncStatus::NotInitialized),
            sync_stats: RwLock::new(PolicySyncStats::default()),
            change_events: RwLock::new(Vec::new()),
            sync_timer: RwLock::new(None),
        }
    }

    /// 使用默认配置创建管理器
    pub fn with_default_config() -> Self {
        Self::new(RemotePolicyConfig::default())
    }

    /// 启动策略同步服务
    #[instrument(skip(self))]
    pub async fn start_sync_service(&self) -> DlpResult<()> {
        info!("Starting policy sync service");
        
        // 设置同步状态
        {
            let mut status = self.sync_status.write().await;
            *status = PolicySyncStatus::Syncing;
        }

        // 执行初始同步
        self.sync_policies().await?;

        // 启动定时同步
        self.start_periodic_sync().await?;

        info!("Policy sync service started successfully");
        Ok(())
    }

    /// 停止策略同步服务
    #[instrument(skip(self))]
    pub async fn stop_sync_service(&self) -> DlpResult<()> {
        info!("Stopping policy sync service");
        
        // 停止定时器
        {
            let mut timer = self.sync_timer.write().await;
            *timer = None;
        }

        // 更新状态
        {
            let mut status = self.sync_status.write().await;
            *status = PolicySyncStatus::Disconnected;
        }

        info!("Policy sync service stopped");
        Ok(())
    }

    /// 手动同步策略
    #[instrument(skip(self))]
    pub async fn sync_policies(&self) -> DlpResult<()> {
        debug!("Starting manual policy sync");
        
        // 更新统计
        {
            let mut stats = self.sync_stats.write().await;
            stats.total_syncs += 1;
            stats.last_sync_time = Some(current_timestamp());
        }

        // 模拟从远程服务器获取策略
        match self.fetch_remote_policies().await {
            Ok((dlp_policies, sensitive_ops_policies, version)) => {
                // 更新本地策略
                {
                    let mut local = self.local_manager.write().await;
                    local.update_dlp_policies(dlp_policies.clone(), version.dlp_rules_version)?;
                    local.update_sensitive_ops_policies(sensitive_ops_policies.clone(), version.sensitive_ops_version)?;
                }

                // 更新统计
                {
                    let mut stats = self.sync_stats.write().await;
                    stats.successful_syncs += 1;
                    stats.last_successful_sync = Some(current_timestamp());
                    stats.dlp_policies_count = dlp_policies.len();
                    stats.sensitive_ops_policies_count = sensitive_ops_policies.len();
                    stats.total_policies = dlp_policies.len() + sensitive_ops_policies.len();
                }

                // 更新状态
                {
                    let mut status = self.sync_status.write().await;
                    *status = PolicySyncStatus::Success;
                }

                // 记录变更事件
                self.record_sync_event(dlp_policies.len(), sensitive_ops_policies.len()).await;

                info!(
                    dlp_count = dlp_policies.len(),
                    sensitive_ops_count = sensitive_ops_policies.len(),
                    "Policy sync completed successfully"
                );
                
                Ok(())
            }
            Err(e) => {
                // 更新统计
                {
                    let mut stats = self.sync_stats.write().await;
                    stats.failed_syncs += 1;
                }

                // 更新状态
                {
                    let mut status = self.sync_status.write().await;
                    *status = PolicySyncStatus::Failed(e.to_string());
                }

                error!(error = %e, "Policy sync failed");
                Err(e)
            }
        }
    }

    /// 获取当前同步状态
    pub async fn get_sync_status(&self) -> PolicySyncStatus {
        self.sync_status.read().await.clone()
    }

    /// 获取同步统计信息
    pub async fn get_sync_stats(&self) -> PolicySyncStats {
        let stats = self.sync_stats.read().await;
        PolicySyncStats {
            total_syncs: stats.total_syncs,
            successful_syncs: stats.successful_syncs,
            failed_syncs: stats.failed_syncs,
            last_sync_time: stats.last_sync_time,
            last_successful_sync: stats.last_successful_sync,
            total_policies: stats.total_policies,
            dlp_policies_count: stats.dlp_policies_count,
            sensitive_ops_policies_count: stats.sensitive_ops_policies_count,
        }
    }

    /// 获取策略变更事件历史
    pub async fn get_change_events(&self, limit: Option<usize>) -> Vec<PolicyChangeEvent> {
        let events = self.change_events.read().await;
        match limit {
            Some(n) => events.iter().rev().take(n).cloned().collect(),
            None => events.clone(),
        }
    }

    /// 获取本地策略管理器的只读引用
    pub async fn get_local_manager(&self) -> tokio::sync::RwLockReadGuard<'_, PolicySyncManager> {
        self.local_manager.read().await
    }

    /// 更新远程配置
    #[instrument(skip(self, config))]
    pub async fn update_remote_config(&self, config: RemotePolicyConfig) -> DlpResult<()> {
        info!("Updating remote policy configuration");
        
        {
            let mut remote_config = self.remote_config.write().await;
            *remote_config = config;
        }

        // 重启同步服务以应用新配置
        self.stop_sync_service().await?;
        self.start_sync_service().await?;

        info!("Remote policy configuration updated successfully");
        Ok(())
    }

    /// 检查是否需要同步
    pub async fn needs_sync(&self) -> DlpResult<bool> {
        let remote_version = self.fetch_remote_version().await?;
        let local = self.local_manager.read().await;
        Ok(local.needs_sync(&remote_version))
    }

    /// 启动定时同步
    async fn start_periodic_sync(&self) -> DlpResult<()> {
        let sync_interval = {
            let config = self.remote_config.read().await;
            config.sync_interval
        };

        // 启动定时同步
        let timer = interval(sync_interval);
        
        {
            let mut sync_timer = self.sync_timer.write().await;
            *sync_timer = Some(timer);
        }

        debug!(
            interval_secs = sync_interval.as_secs(),
            "Periodic sync timer started"
        );

        Ok(())
    }

    /// 从远程服务器获取策略
    async fn fetch_remote_policies(&self) -> DlpResult<(Vec<DlpPolicy>, Vec<SensitiveOpPolicy>, PolicyVersion)> {
        debug!("Fetching policies from remote server");
        
        #[cfg(test)]
        {
            // 测试环境：返回模拟数据
            use crate::policy_sync::{DlpPolicy, SensitiveOpPolicy, PolicyVersion};
            
            let dlp_policies = vec![
                DlpPolicy {
                    id: "test-dlp-1".to_string(),
                    pattern: r"\d{17}[\dXx]".to_string(),
                    replacement: "***************".to_string(),
                    severity: "high".to_string(),
                },
                DlpPolicy {
                    id: "test-dlp-2".to_string(),
                    pattern: r"1[3-9]\d{9}".to_string(),
                    replacement: "***********".to_string(),
                    severity: "medium".to_string(),
                },
            ];
            
            let sensitive_ops_policies = vec![
                SensitiveOpPolicy {
                    id: "test-ops-1".to_string(),
                    operation: "file_upload".to_string(),
                    requires_approval: true,
                    risk_level: "high".to_string(),
                },
            ];
            
            let version = PolicyVersion {
                dlp_rules_version: 1,
                sensitive_ops_version: 1,
                last_sync: current_timestamp(),
            };
            
            return Ok((dlp_policies, sensitive_ops_policies, version));
        }
        
        #[cfg(not(test))]
        {
            // 生产环境：真实的 HTTP 调用
            let config = self.remote_config.read().await;
            
            // 创建 HTTP 客户端
            let client = reqwest::Client::builder()
                .timeout(config.connection_timeout)
                .build()
                .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to create HTTP client: {}", e)))?;
            
            // 发送请求（带重试）
            let mut last_error = None;
            for attempt in 0..config.max_retries {
                match self.fetch_policies_with_client(&client, &config).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        last_error = Some(e);
                        
                        if attempt < config.max_retries - 1 {
                            warn!(
                                attempt = attempt + 1,
                                max_retries = config.max_retries,
                                error = %last_error.as_ref().unwrap(),
                                "Policy fetch failed, retrying..."
                            );
                            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                            continue;
                        }
                    }
                }
            }
            
            Err(last_error.unwrap_or_else(|| 
                crate::dlp::DlpError::Sanitization("Failed to fetch policies".to_string())
            ))
        }
    }

    /// 使用 HTTP 客户端获取策略（实际实现）
    async fn fetch_policies_with_client(
        &self,
        client: &reqwest::Client,
        config: &RemotePolicyConfig,
    ) -> DlpResult<(Vec<DlpPolicy>, Vec<SensitiveOpPolicy>, PolicyVersion)> {
        // 1. 获取 DLP 规则
        let dlp_url = format!("{}/api/policies/dlp", config.server_url);
        let dlp_response = client
            .get(&dlp_url)
            .header("Authorization", format!("Bearer {}", config.auth_token))
            .send()
            .await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to fetch DLP policies: {}", e)))?;

        if !dlp_response.status().is_success() {
            let status = dlp_response.status();
            let error_text = dlp_response.text().await.unwrap_or_default();
            return Err(crate::dlp::DlpError::Sanitization(format!("HTTP {}: {}", status, error_text)));
        }

        let dlp_rules_raw: Vec<serde_json::Value> = dlp_response.json().await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to parse DLP response: {}", e)))?;

        // 2. 获取敏感操作规则
        let sensitive_ops_url = format!("{}/api/policies/sensitive-ops", config.server_url);
        let sensitive_ops_response = client
            .get(&sensitive_ops_url)
            .header("Authorization", format!("Bearer {}", config.auth_token))
            .send()
            .await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to fetch sensitive ops policies: {}", e)))?;

        if !sensitive_ops_response.status().is_success() {
            let status = sensitive_ops_response.status();
            let error_text = sensitive_ops_response.text().await.unwrap_or_default();
            return Err(crate::dlp::DlpError::Sanitization(format!("HTTP {}: {}", status, error_text)));
        }

        let sensitive_ops_raw: Vec<serde_json::Value> = sensitive_ops_response.json().await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to parse sensitive ops response: {}", e)))?;

        // 3. 获取版本信息
        let version_url = format!("{}/api/policies/version", config.server_url);
        let version_response = client
            .get(&version_url)
            .header("Authorization", format!("Bearer {}", config.auth_token))
            .send()
            .await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to fetch version: {}", e)))?;

        if !version_response.status().is_success() {
            let status = version_response.status();
            let error_text = version_response.text().await.unwrap_or_default();
            return Err(crate::dlp::DlpError::Sanitization(format!("HTTP {}: {}", status, error_text)));
        }

        let version_info: serde_json::Value = version_response.json().await
            .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to parse version: {}", e)))?;

        // 4. 转换为本地格式
        let dlp_policies = self.convert_dlp_rules(dlp_rules_raw)?;
        let sensitive_ops_policies = self.convert_sensitive_ops_rules(sensitive_ops_raw)?;
        let version = PolicyVersion {
            dlp_rules_version: version_info["dlp_rules_version"].as_u64().unwrap_or(0),
            sensitive_ops_version: version_info["sensitive_ops_version"].as_u64().unwrap_or(0),
            last_sync: current_timestamp(),
        };

        Ok((dlp_policies, sensitive_ops_policies, version))
    }

    /// 转换 Admin Backend 的 DLP 规则格式为本地格式
    fn convert_dlp_rules(&self, rules_raw: Vec<serde_json::Value>) -> DlpResult<Vec<DlpPolicy>> {
        let mut policies = Vec::new();
        
        for rule_raw in rules_raw {
            let policy = DlpPolicy {
                id: rule_raw["id"].as_str().unwrap_or("").to_string(),
                pattern: rule_raw["pattern"].as_str().unwrap_or("").to_string(),
                replacement: rule_raw["replacement"].as_str().unwrap_or("").to_string(),
                severity: rule_raw["severity"].as_str().unwrap_or("medium").to_string(),
            };
            policies.push(policy);
        }
        
        Ok(policies)
    }

    /// 转换 Admin Backend 的敏感操作规则格式为本地格式
    fn convert_sensitive_ops_rules(&self, rules_raw: Vec<serde_json::Value>) -> DlpResult<Vec<SensitiveOpPolicy>> {
        let mut policies = Vec::new();
        
        for rule_raw in rules_raw {
            let policy = SensitiveOpPolicy {
                id: rule_raw["id"].as_str().unwrap_or("").to_string(),
                operation: rule_raw["operation_type"].as_str().unwrap_or("").to_string(),
                requires_approval: rule_raw["requires_approval"].as_bool().unwrap_or(false),
                risk_level: rule_raw["risk_level"].as_str().unwrap_or("medium").to_string(),
            };
            policies.push(policy);
        }
        
        Ok(policies)
    }

    /// 获取远程版本信息
    async fn fetch_remote_version(&self) -> DlpResult<PolicyVersion> {
        debug!("Fetching remote policy version");
        
        #[cfg(test)]
        {
            // 测试环境：返回模拟版本
            use crate::policy_sync::PolicyVersion;
            
            return Ok(PolicyVersion {
                dlp_rules_version: 1,
                sensitive_ops_version: 1,
                last_sync: current_timestamp(),
            });
        }
        
        #[cfg(not(test))]
        {
            // 生产环境：真实的 HTTP 调用
            let config = self.remote_config.read().await;
            let url = format!("{}/api/policies/version", config.server_url);
            
            let client = reqwest::Client::builder()
                .timeout(config.connection_timeout)
                .build()
                .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to create HTTP client: {}", e)))?;
            
            let response = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", config.auth_token))
                .send()
                .await
                .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to fetch version: {}", e)))?;
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(crate::dlp::DlpError::Sanitization(format!("HTTP {}: {}", status, error_text)));
            }
            
            let version_info: serde_json::Value = response.json().await
                .map_err(|e| crate::dlp::DlpError::Sanitization(format!("Failed to parse version: {}", e)))?;
            
            Ok(PolicyVersion {
                dlp_rules_version: version_info["dlp_rules_version"].as_u64().unwrap_or(0),
                sensitive_ops_version: version_info["sensitive_ops_version"].as_u64().unwrap_or(0),
                last_sync: current_timestamp(),
            })
        }
    }

    /// 记录同步事件
    async fn record_sync_event(&self, dlp_count: usize, sensitive_ops_count: usize) {
        let event = PolicyChangeEvent {
            id: Uuid::new_v4(),
            event_type: PolicyChangeType::Updated,
            policy_id: "sync_all".to_string(),
            changes: serde_json::json!({
                "dlp_policies_count": dlp_count,
                "sensitive_ops_policies_count": sensitive_ops_count,
                "sync_type": "full_sync"
            }),
            timestamp: current_timestamp(),
            operator: "system".to_string(),
        };

        let mut events = self.change_events.write().await;
        events.push(event);

        // 保持事件历史在合理范围内（最多1000条）
        if events.len() > 1000 {
            let excess = events.len() - 1000;
            events.drain(0..excess);
        }
    }
}

/// 获取当前时间戳
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_enterprise_policy_sync_manager_creation() {
        let config = RemotePolicyConfig::default();
        let manager = EnterprisePolicySyncManager::new(config);
        
        let status = manager.get_sync_status().await;
        assert!(matches!(status, PolicySyncStatus::NotInitialized));
        
        let stats = manager.get_sync_stats().await;
        assert_eq!(stats.total_syncs, 0);
        assert_eq!(stats.successful_syncs, 0);
        assert_eq!(stats.failed_syncs, 0);
    }

    #[tokio::test]
    async fn test_sync_policies() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        let result = manager.sync_policies().await;
        assert!(result.is_ok());
        
        let status = manager.get_sync_status().await;
        assert!(matches!(status, PolicySyncStatus::Success));
        
        let stats = manager.get_sync_stats().await;
        assert_eq!(stats.total_syncs, 1);
        assert_eq!(stats.successful_syncs, 1);
        assert_eq!(stats.failed_syncs, 0);
        assert!(stats.last_sync_time.is_some());
        assert!(stats.last_successful_sync.is_some());
        assert_eq!(stats.dlp_policies_count, 2);
        assert_eq!(stats.sensitive_ops_policies_count, 1);
        assert_eq!(stats.total_policies, 3);
    }

    #[tokio::test]
    async fn test_get_change_events() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 执行同步以生成事件
        manager.sync_policies().await.unwrap();
        
        let events = manager.get_change_events(None).await;
        assert_eq!(events.len(), 1);
        
        let event = &events[0];
        assert!(matches!(event.event_type, PolicyChangeType::Updated));
        assert_eq!(event.policy_id, "sync_all");
        assert_eq!(event.operator, "system");
    }

    #[tokio::test]
    async fn test_get_change_events_with_limit() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 执行多次同步
        for _ in 0..3 {
            manager.sync_policies().await.unwrap();
        }
        
        let events = manager.get_change_events(Some(2)).await;
        assert_eq!(events.len(), 2);
        
        // 应该返回最新的2个事件（倒序）
        assert!(events[0].timestamp >= events[1].timestamp);
    }

    #[tokio::test]
    async fn test_needs_sync() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 初始状态应该需要同步
        let needs_sync = manager.needs_sync().await.unwrap();
        assert!(needs_sync);
        
        // 同步后应该不需要同步
        manager.sync_policies().await.unwrap();
        let needs_sync = manager.needs_sync().await.unwrap();
        assert!(!needs_sync);
    }

    #[tokio::test]
    async fn test_update_remote_config() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        let new_config = RemotePolicyConfig {
            server_url: "https://new-server.example.com".to_string(),
            sync_interval: Duration::from_secs(600),
            auth_token: "new-token".to_string(),
            enable_realtime: false,
            connection_timeout: Duration::from_secs(60),
            max_retries: 5,
        };
        
        let result = manager.update_remote_config(new_config.clone()).await;
        assert!(result.is_ok());
        
        // 验证配置已更新
        let current_config = manager.remote_config.read().await;
        assert_eq!(current_config.server_url, new_config.server_url);
        assert_eq!(current_config.sync_interval, new_config.sync_interval);
        assert_eq!(current_config.auth_token, new_config.auth_token);
        assert_eq!(current_config.enable_realtime, new_config.enable_realtime);
    }

    #[tokio::test]
    async fn test_start_and_stop_sync_service() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 启动服务
        let result = manager.start_sync_service().await;
        assert!(result.is_ok());
        
        let status = manager.get_sync_status().await;
        assert!(matches!(status, PolicySyncStatus::Success));
        
        // 停止服务
        let result = manager.stop_sync_service().await;
        assert!(result.is_ok());
        
        let status = manager.get_sync_status().await;
        assert!(matches!(status, PolicySyncStatus::Disconnected));
    }

    #[tokio::test]
    async fn test_get_local_manager() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 执行同步以填充策略
        manager.sync_policies().await.unwrap();
        
        let local = manager.get_local_manager().await;
        assert_eq!(local.get_dlp_policies().len(), 2);
        assert_eq!(local.get_sensitive_ops_policies().len(), 1);
    }

    #[tokio::test]
    async fn test_multiple_sync_operations() {
        let manager = EnterprisePolicySyncManager::with_default_config();
        
        // 执行多次同步
        for i in 0..5 {
            let result = manager.sync_policies().await;
            assert!(result.is_ok(), "Sync {} failed", i + 1);
        }
        
        let stats = manager.get_sync_stats().await;
        assert_eq!(stats.total_syncs, 5);
        assert_eq!(stats.successful_syncs, 5);
        assert_eq!(stats.failed_syncs, 0);
        
        let events = manager.get_change_events(None).await;
        assert_eq!(events.len(), 5);
    }

    #[tokio::test]
    async fn test_concurrent_sync_operations() {
        let manager = std::sync::Arc::new(EnterprisePolicySyncManager::with_default_config());
        
        // 并发执行同步操作
        let mut handles = Vec::new();
        for _ in 0..3 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                manager_clone.sync_policies().await
            });
            handles.push(handle);
        }
        
        // 等待所有操作完成
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        let stats = manager.get_sync_stats().await;
        assert_eq!(stats.total_syncs, 3);
        assert_eq!(stats.successful_syncs, 3);
    }

    #[test]
    fn test_current_timestamp() {
        let timestamp1 = current_timestamp();
        // Sleep 至少1秒,因为 current_timestamp 返回秒级时间戳
        std::thread::sleep(std::time::Duration::from_secs(1));
        let timestamp2 = current_timestamp();
        
        assert!(timestamp2 > timestamp1);
        assert_eq!(timestamp2 - timestamp1, 1); // 应该正好相差1秒
    }

    #[test]
    fn test_remote_policy_config_default() {
        let config = RemotePolicyConfig::default();
        
        assert_eq!(config.server_url, "https://policy.example.com");
        assert_eq!(config.sync_interval, Duration::from_secs(300));
        assert!(config.auth_token.is_empty());
        assert!(config.enable_realtime);
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_policy_sync_stats_default() {
        let stats = PolicySyncStats::default();
        
        assert_eq!(stats.total_syncs, 0);
        assert_eq!(stats.successful_syncs, 0);
        assert_eq!(stats.failed_syncs, 0);
        assert!(stats.last_sync_time.is_none());
        assert!(stats.last_successful_sync.is_none());
        assert_eq!(stats.total_policies, 0);
        assert_eq!(stats.dlp_policies_count, 0);
        assert_eq!(stats.sensitive_ops_policies_count, 0);
    }
}