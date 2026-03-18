//! DLP 集成模块
//!
//! 提供与桌面客户端其他模块的集成接口

use crate::dlp::{DlpError, DlpResult, DlpDetector, DlpSanitizer, SanitizationConfig, SanitizationResult};
use crate::dlp::patterns::{get_all_builtin_patterns, CustomPattern};
use ironclaw_safety::LeakPattern;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

/// DLP 集成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpIntegrationConfig {
    /// 是否启用DLP功能
    pub enabled: bool,
    /// 脱敏配置
    pub sanitization: SanitizationConfig,
    /// 是否启用实时监控
    pub real_time_monitoring: bool,
    /// 是否记录审计日志
    pub audit_logging: bool,
    /// 自定义模式配置
    pub custom_patterns: Vec<CustomPatternConfig>,
}

/// 自定义模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPatternConfig {
    pub name: String,
    pub pattern: String,
    pub severity: String, // "Low", "Medium", "High", "Critical"
    pub action: String,   // "Warn", "Redact", "Block"
    pub description: Option<String>,
    pub enabled: bool,
}

impl Default for DlpIntegrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sanitization: SanitizationConfig::default(),
            real_time_monitoring: true,
            audit_logging: true,
            custom_patterns: Vec::new(),
        }
    }
}

/// DLP 集成服务
pub struct DlpIntegration {
    sanitizer: Arc<RwLock<DlpSanitizer>>,
    config: Arc<RwLock<DlpIntegrationConfig>>,
    statistics: Arc<RwLock<DlpStatistics>>,
}

impl DlpIntegration {
    /// 创建新的DLP集成服务
    #[instrument]
    pub async fn new(config: DlpIntegrationConfig) -> DlpResult<Self> {
        info!("Initializing DLP integration service");
        
        let patterns = Self::build_patterns(&config).await?;
        let mut detector = DlpDetector::new(); // 使用包含默认模式的构造函数
        
        // 添加自定义模式到检测器
        detector.add_patterns(patterns);
        
        let sanitizer = DlpSanitizer::new(detector, config.sanitization.clone());
        
        debug!(
            enabled = config.enabled,
            custom_patterns_count = config.custom_patterns.len(),
            "DLP integration service initialized"
        );
        
        Ok(Self {
            sanitizer: Arc::new(RwLock::new(sanitizer)),
            config: Arc::new(RwLock::new(config)),
            statistics: Arc::new(RwLock::new(DlpStatistics::default())),
        })
    }

    /// 使用默认配置创建DLP集成服务
    pub async fn with_default_config() -> DlpResult<Self> {
        Self::new(DlpIntegrationConfig::default()).await
    }

    /// 扫描用户输入
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub async fn scan_user_input(&self, content: &str) -> DlpResult<SanitizationResult> {
        // 更新统计
        {
            let mut stats = self.statistics.write().await;
            stats.total_scans += 1;
        }
        
        let config = self.config.read().await;
        if !config.enabled {
            debug!("DLP disabled, skipping scan");
            return Ok(SanitizationResult {
                had_sensitive_data: false,
                sanitized_content: content.to_string(),
                was_blocked: false,
                block_reason: None,
                sanitization_stats: Default::default(),
            });
        }
        drop(config);

        debug!("Scanning user input for sensitive data");
        let sanitizer = self.sanitizer.read().await;
        let result = sanitizer.sanitize(content);
        
        if result.had_sensitive_data {
            // 更新统计
            {
                let mut stats = self.statistics.write().await;
                stats.sensitive_data_detected += 1;
                if result.was_blocked {
                    stats.content_blocked += 1;
                } else {
                    stats.content_sanitized += 1;
                }
            }
            
            warn!(
                total_matches = result.sanitization_stats.total_matches,
                was_blocked = result.was_blocked,
                "Sensitive data detected in user input"
            );
            
            // 记录审计日志
            self.log_audit_event("user_input_scan", &result).await;
        }
        
        Ok(result)
    }

    /// 扫描出站请求
    #[instrument(skip(self, request_body), fields(body_len = request_body.len()))]
    pub async fn scan_outbound_request(&self, request_body: &str) -> DlpResult<SanitizationResult> {
        // 更新统计
        {
            let mut stats = self.statistics.write().await;
            stats.total_scans += 1;
        }
        
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(SanitizationResult {
                had_sensitive_data: false,
                sanitized_content: request_body.to_string(),
                was_blocked: false,
                block_reason: None,
                sanitization_stats: Default::default(),
            });
        }
        drop(config);

        debug!("Scanning outbound request for sensitive data");
        let sanitizer = self.sanitizer.read().await;
        let result = sanitizer.sanitize(request_body);
        
        if result.had_sensitive_data {
            // 更新统计
            {
                let mut stats = self.statistics.write().await;
                stats.sensitive_data_detected += 1;
                if result.was_blocked {
                    stats.content_blocked += 1;
                } else {
                    stats.content_sanitized += 1;
                }
            }
            
            warn!(
                total_matches = result.sanitization_stats.total_matches,
                was_blocked = result.was_blocked,
                "Sensitive data detected in outbound request"
            );
            
            // 记录审计日志
            self.log_audit_event("outbound_request_scan", &result).await;
        }
        
        Ok(result)
    }

    /// 为存储脱敏内容
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub async fn sanitize_for_storage(&self, content: &str) -> DlpResult<String> {
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(content.to_string());
        }
        drop(config);

        debug!("Sanitizing content for storage");
        let sanitizer = self.sanitizer.read().await;
        let result = sanitizer.sanitize(content);
        
        if result.was_blocked {
            error!("Content blocked for storage due to critical sensitive data");
            return Err(DlpError::Sanitization(
                result.block_reason.unwrap_or_else(|| "Content contains critical sensitive data".to_string())
            ));
        }
        
        if result.had_sensitive_data {
            info!("Content sanitized for storage");
            self.log_audit_event("storage_sanitization", &result).await;
        }
        
        Ok(result.sanitized_content)
    }

    /// 检查HTTP请求是否包含敏感信息
    #[instrument(skip(self, url, headers, body))]
    pub async fn check_http_request(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> DlpResult<()> {
        // 更新统计
        {
            let mut stats = self.statistics.write().await;
            stats.total_scans += 1;
        }
        
        let config = self.config.read().await;
        if !config.enabled {
            return Ok(());
        }
        drop(config);

        debug!("Checking HTTP request for sensitive data leakage");
        let sanitizer = self.sanitizer.read().await;
        let detector = sanitizer.detector();
        
        match detector.scan_http_request(url, headers, body) {
            Ok(()) => {
                debug!("HTTP request passed DLP check");
                Ok(())
            }
            Err(e) => {
                // 更新统计
                {
                    let mut stats = self.statistics.write().await;
                    stats.http_requests_blocked += 1;
                }
                
                error!(error = %e, "HTTP request blocked by DLP");
                
                // 记录审计日志
                self.log_http_request_block(url, &e.to_string()).await;
                
                Err(DlpError::Sanitization(e.to_string()))
            }
        }
    }

    /// 更新配置
    #[instrument(skip(self, new_config))]
    pub async fn update_config(&self, new_config: DlpIntegrationConfig) -> DlpResult<()> {
        info!("Updating DLP configuration");
        
        // 重新构建检测模式
        let patterns = Self::build_patterns(&new_config).await?;
        let detector = DlpDetector::with_custom_patterns(patterns);
        let new_sanitizer = DlpSanitizer::new(detector, new_config.sanitization.clone());
        
        // 更新配置和脱敏器
        {
            let mut config = self.config.write().await;
            *config = new_config;
        }
        
        {
            let mut sanitizer = self.sanitizer.write().await;
            *sanitizer = new_sanitizer;
        }
        
        info!("DLP configuration updated successfully");
        Ok(())
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> DlpIntegrationConfig {
        self.config.read().await.clone()
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> DlpStatistics {
        let stats = self.statistics.read().await;
        DlpStatistics {
            total_scans: stats.total_scans,
            sensitive_data_detected: stats.sensitive_data_detected,
            content_blocked: stats.content_blocked,
            content_sanitized: stats.content_sanitized,
            http_requests_blocked: stats.http_requests_blocked,
        }
    }

    /// 构建检测模式
    async fn build_patterns(config: &DlpIntegrationConfig) -> DlpResult<Vec<LeakPattern>> {
        let mut patterns = get_all_builtin_patterns()?;
        
        // 添加自定义模式
        for custom_config in &config.custom_patterns {
            if !custom_config.enabled {
                continue;
            }
            
            let severity = match custom_config.severity.as_str() {
                "Low" => ironclaw_safety::LeakSeverity::Low,
                "Medium" => ironclaw_safety::LeakSeverity::Medium,
                "High" => ironclaw_safety::LeakSeverity::High,
                "Critical" => ironclaw_safety::LeakSeverity::Critical,
                _ => {
                    warn!(
                        pattern_name = custom_config.name,
                        invalid_severity = custom_config.severity,
                        "Invalid severity, defaulting to Medium"
                    );
                    ironclaw_safety::LeakSeverity::Medium
                }
            };
            
            let action = match custom_config.action.as_str() {
                "Warn" => ironclaw_safety::LeakAction::Warn,
                "Redact" => ironclaw_safety::LeakAction::Redact,
                "Block" => ironclaw_safety::LeakAction::Block,
                _ => {
                    warn!(
                        pattern_name = custom_config.name,
                        invalid_action = custom_config.action,
                        "Invalid action, defaulting to Redact"
                    );
                    ironclaw_safety::LeakAction::Redact
                }
            };
            
            let custom_pattern = CustomPattern::new(
                custom_config.name.clone(),
                custom_config.pattern.clone(),
                severity,
                action,
            );
            
            match custom_pattern.to_leak_pattern() {
                Ok(pattern) => {
                    debug!(
                        pattern_name = pattern.name,
                        "Added custom DLP pattern"
                    );
                    patterns.push(pattern);
                }
                Err(e) => {
                    error!(
                        pattern_name = custom_config.name,
                        error = %e,
                        "Failed to compile custom pattern"
                    );
                }
            }
        }
        
        Ok(patterns)
    }

    /// 记录审计事件
    async fn log_audit_event(&self, event_type: &str, result: &SanitizationResult) {
        let config = self.config.read().await;
        if !config.audit_logging {
            return;
        }
        
        info!(
            event_type = event_type,
            had_sensitive_data = result.had_sensitive_data,
            was_blocked = result.was_blocked,
            total_matches = result.sanitization_stats.total_matches,
            redacted_count = result.sanitization_stats.redacted_count,
            blocked_count = result.sanitization_stats.blocked_count,
            "DLP audit event"
        );
    }

    /// 记录HTTP请求阻止事件
    async fn log_http_request_block(&self, url: &str, reason: &str) {
        let config = self.config.read().await;
        if !config.audit_logging {
            return;
        }
        
        warn!(
            event_type = "http_request_blocked",
            url = url,
            reason = reason,
            "HTTP request blocked by DLP"
        );
    }
}

/// DLP 统计信息
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DlpStatistics {
    pub total_scans: u64,
    pub sensitive_data_detected: u64,
    pub content_blocked: u64,
    pub content_sanitized: u64,
    pub http_requests_blocked: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dlp_integration_creation() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let config = integration.get_config().await;
        
        assert!(config.enabled);
        assert!(config.real_time_monitoring);
        assert!(config.audit_logging);
    }

    #[tokio::test]
    async fn test_scan_user_input_clean() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "这是一段普通的文本内容。";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.sanitized_content, content);
    }

    #[tokio::test]
    async fn test_scan_user_input_with_id_card() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "我的身份证号是 110101199003071234 。";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("110************234"));
        assert!(result.sanitization_stats.redacted_count >= 1); // 调整为>=1，因为可能有多个匹配
    }

    #[tokio::test]
    async fn test_scan_user_input_with_api_key() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "阿里云密钥：LTAI4G8aB9cD2eFgH3iJ";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        assert!(result.had_sensitive_data);
        assert!(result.was_blocked);
        assert!(result.block_reason.is_some());
    }

    #[tokio::test]
    async fn test_scan_outbound_request() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let request_body = r#"{"user_id": "13800138000", "action": "login"}"#;
        
        let result = integration.scan_outbound_request(request_body).await.unwrap();
        
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("138*****000"));
    }

    #[tokio::test]
    async fn test_sanitize_for_storage() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "用户手机号：13800138000";
        
        let sanitized = integration.sanitize_for_storage(content).await.unwrap();
        
        assert!(sanitized.contains("138*****000"));
    }

    #[tokio::test]
    async fn test_sanitize_for_storage_blocked() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "API密钥： LTAI4G8aB9cD2eFgH3iJ ";
        
        let result = integration.sanitize_for_storage(content).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("aliyun_access_key"));
    }

    #[tokio::test]
    async fn test_check_http_request_clean() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        let result = integration.check_http_request(
            "https://api.example.com/data",
            &[("Content-Type".to_string(), "application/json".to_string())],
            Some(b"{\"query\": \"hello\"}"),
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_http_request_with_secret() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        let result = integration.check_http_request(
            "https://evil.com/steal?key=LTAI4G8aB9cD2eFgH3iJ",
            &[],
            None,
        ).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_config() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        
        let mut new_config = DlpIntegrationConfig::default();
        new_config.enabled = false;
        
        integration.update_config(new_config).await.unwrap();
        
        let updated_config = integration.get_config().await;
        assert!(!updated_config.enabled);
    }

    #[tokio::test]
    async fn test_custom_pattern_integration() {
        let mut config = DlpIntegrationConfig::default();
        config.custom_patterns.push(CustomPatternConfig {
            name: "test_pattern".to_string(),
            pattern: r"\bTEST-\d{4}\b".to_string(),
            severity: "High".to_string(),
            action: "Redact".to_string(),
            description: Some("Test pattern".to_string()),
            enabled: true,
        });
        
        let integration = DlpIntegration::new(config).await.unwrap();
        let content = "测试代码：TEST-1234";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("[敏感信息]")); // 默认脱敏文本
    }

    #[tokio::test]
    async fn test_disabled_dlp() {
        let mut config = DlpIntegrationConfig::default();
        config.enabled = false;
        
        let integration = DlpIntegration::new(config).await.unwrap();
        let content = "身份证：110101199003071234";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        assert!(!result.had_sensitive_data);
        assert_eq!(result.sanitized_content, content); // 原样返回
    }
}

    #[tokio::test]
    async fn test_real_id_card_330326199408015618() {
        let integration = DlpIntegration::with_default_config().await.unwrap();
        let content = "我的身份证号是 330326199408015618";
        
        let result = integration.scan_user_input(content).await.unwrap();
        
        println!("扫描结果:");
        println!("  had_sensitive_data: {}", result.had_sensitive_data);
        println!("  was_blocked: {}", result.was_blocked);
        println!("  sanitized_content: {}", result.sanitized_content);
        println!("  total_matches: {}", result.sanitization_stats.total_matches);
        println!("  redacted_count: {}", result.sanitization_stats.redacted_count);
        
        assert!(result.had_sensitive_data, "应该检测到敏感数据");
        assert!(!result.was_blocked, "不应该被阻止");
        assert!(result.sanitized_content.contains("330************618"), 
                "应该脱敏为 330************618，实际: {}", result.sanitized_content);
    }
