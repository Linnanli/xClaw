//! DLP 检测器核心实现
//!
//! 基于 ironclaw_safety crate 的 LeakDetector，扩展支持中国特色敏感信息检测

use crate::dlp::{DlpError, DlpResult};
use ironclaw_safety::{LeakDetector, LeakScanResult, LeakMatch as SafetyLeakMatch, LeakSeverity as SafetyLeakSeverity, LeakAction as SafetyLeakAction};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use tracing::{debug, instrument, warn};

/// DLP 检测严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DlpSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl From<SafetyLeakSeverity> for DlpSeverity {
    fn from(severity: SafetyLeakSeverity) -> Self {
        match severity {
            SafetyLeakSeverity::Low => DlpSeverity::Low,
            SafetyLeakSeverity::Medium => DlpSeverity::Medium,
            SafetyLeakSeverity::High => DlpSeverity::High,
            SafetyLeakSeverity::Critical => DlpSeverity::Critical,
        }
    }
}

/// DLP 检测动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DlpAction {
    /// 仅记录警告
    Warn,
    /// 脱敏处理
    Redact,
    /// 完全阻止
    Block,
}

impl From<SafetyLeakAction> for DlpAction {
    fn from(action: SafetyLeakAction) -> Self {
        match action {
            SafetyLeakAction::Warn => DlpAction::Warn,
            SafetyLeakAction::Redact => DlpAction::Redact,
            SafetyLeakAction::Block => DlpAction::Block,
        }
    }
}

/// DLP 匹配结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpMatch {
    /// 模式名称
    pub pattern_name: String,
    /// 严重程度
    pub severity: DlpSeverity,
    /// 处理动作
    pub action: DlpAction,
    /// 匹配位置
    pub location: Range<usize>,
    /// 脱敏预览
    pub masked_preview: String,
    /// 原始匹配文本（仅用于测试，生产环境不暴露）
    #[serde(skip_serializing)]
    pub original_text: Option<String>,
}

impl From<SafetyLeakMatch> for DlpMatch {
    fn from(leak_match: SafetyLeakMatch) -> Self {
        Self {
            pattern_name: leak_match.pattern_name,
            severity: leak_match.severity.into(),
            action: leak_match.action.into(),
            location: leak_match.location,
            masked_preview: leak_match.masked_preview,
            original_text: None, // 安全考虑，不保存原始文本
        }
    }
}

/// DLP 检测结果
#[derive(Debug, Serialize, Deserialize)]
pub struct DlpDetectionResult {
    /// 是否检测到敏感信息
    pub has_sensitive_data: bool,
    /// 是否应该阻止
    pub should_block: bool,
    /// 检测到的匹配项
    pub matches: Vec<DlpMatch>,
    /// 脱敏后的内容（如果有脱敏操作）
    pub sanitized_content: Option<String>,
    /// 最高严重程度
    pub max_severity: Option<DlpSeverity>,
}

impl From<LeakScanResult> for DlpDetectionResult {
    fn from(scan_result: LeakScanResult) -> Self {
        let matches: Vec<DlpMatch> = scan_result.matches.into_iter().map(Into::into).collect();
        let max_severity = matches.iter().map(|m| m.severity).max();
        
        Self {
            has_sensitive_data: !matches.is_empty(),
            should_block: scan_result.should_block,
            matches,
            sanitized_content: scan_result.redacted_content,
            max_severity,
        }
    }
}

/// DLP 检测器
pub struct DlpDetector {
    /// 底层安全检测器
    leak_detector: LeakDetector,
}

impl DlpDetector {
    /// 创建新的 DLP 检测器
    pub fn new() -> Self {
        // 获取所有内置模式（包括中国特色模式）
        let custom_patterns = crate::dlp::patterns::get_all_builtin_patterns()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load builtin patterns: {}, using empty list", e);
                Vec::new()
            });
        
        // 创建默认检测器并添加我们的自定义模式
        let mut leak_detector = LeakDetector::new(); // 包含默认模式
        
        // 添加我们的自定义模式
        for pattern in custom_patterns {
            leak_detector.add_pattern(pattern);
        }
        
        Self { leak_detector }
    }

    /// 使用自定义模式创建检测器
    pub fn with_custom_patterns(patterns: Vec<ironclaw_safety::LeakPattern>) -> Self {
        Self {
            leak_detector: LeakDetector::with_patterns(patterns),
        }
    }

    /// 扫描文本中的敏感信息
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub fn scan(&self, content: &str) -> DlpDetectionResult {
        debug!("Starting DLP scan");
        
        let scan_result = self.leak_detector.scan(content);
        let result = DlpDetectionResult::from(scan_result);
        
        if result.has_sensitive_data {
            warn!(
                matches_count = result.matches.len(),
                max_severity = ?result.max_severity,
                should_block = result.should_block,
                "Sensitive data detected"
            );
        } else {
            debug!("No sensitive data detected");
        }
        
        result
    }

    /// 扫描并清理内容
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub fn scan_and_clean(&self, content: &str) -> DlpResult<String> {
        debug!("Starting DLP scan and clean");
        
        self.leak_detector
            .scan_and_clean(content)
            .map_err(|e| DlpError::Sanitization(e.to_string()))
    }

    /// 扫描 HTTP 请求
    #[instrument(skip(self, url, headers, body))]
    pub fn scan_http_request(
        &self,
        url: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
    ) -> DlpResult<()> {
        debug!("Scanning HTTP request for sensitive data");
        
        self.leak_detector
            .scan_http_request(url, headers, body)
            .map_err(|e| DlpError::Sanitization(e.to_string()))
    }

    /// 获取模式数量
    pub fn pattern_count(&self) -> usize {
        self.leak_detector.pattern_count()
    }

    /// 添加自定义模式
    pub fn add_patterns(&mut self, patterns: Vec<ironclaw_safety::LeakPattern>) {
        for pattern in patterns {
            self.leak_detector.add_pattern(pattern);
        }
    }
}

impl Default for DlpDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlp_detector_creation() {
        let detector = DlpDetector::new();
        println!("Pattern count: {}", detector.pattern_count());
        assert!(detector.pattern_count() > 0);
    }

    #[test]
    fn test_scan_chinese_id_card_in_sentence() {
        let detector = DlpDetector::new();
        let content = "我的身份证号是 110101199003071234 ，请保密。";
        
        let result = detector.scan(content);
        
        println!("Matches found: {}", result.matches.len());
        for (i, m) in result.matches.iter().enumerate() {
            println!("Match {}: pattern={}, severity={:?}, action={:?}, location={:?}, preview={}", 
                i, m.pattern_name, m.severity, m.action, m.location, m.masked_preview);
        }
        
        assert!(result.has_sensitive_data);
        assert!(!result.should_block);
        assert!(!result.matches.is_empty());
        
        let id_match = result.matches.iter()
            .find(|m| m.pattern_name.contains("chinese_id_card"));
        assert!(id_match.is_some(), "Should find chinese_id_card match");
        
        let id_match = id_match.unwrap();
        assert_eq!(id_match.severity, DlpSeverity::High);
        assert_eq!(id_match.action, DlpAction::Redact);
    }

    #[test]
    fn test_scan_openai_api_key() {
        let detector = DlpDetector::new();
        let content = "My API key is sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";
        
        let result = detector.scan(content);
        
        assert!(result.has_sensitive_data);
        assert!(result.should_block);
        assert!(!result.matches.is_empty());
        
        let first_match = &result.matches[0];
        assert_eq!(first_match.pattern_name, "openai_api_key");
        assert_eq!(first_match.severity, DlpSeverity::Critical);
        assert_eq!(first_match.action, DlpAction::Block);
        assert!(first_match.masked_preview.contains("sk-p"));
        assert!(first_match.masked_preview.contains("*"));
    }

    #[test]
    fn test_scan_github_token() {
        let detector = DlpDetector::new();
        let content = "GitHub token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        
        let result = detector.scan(content);
        
        assert!(result.has_sensitive_data);
        assert!(result.should_block);
        
        let github_match = result.matches.iter()
            .find(|m| m.pattern_name == "github_token");
        assert!(github_match.is_some());
    }

    #[test]
    fn test_scan_aws_access_key() {
        let detector = DlpDetector::new();
        let content = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        
        let result = detector.scan(content);
        
        assert!(result.has_sensitive_data);
        assert!(result.should_block);
        
        let aws_match = result.matches.iter()
            .find(|m| m.pattern_name == "aws_access_key");
        assert!(aws_match.is_some());
    }

    #[test]
    fn test_scan_bearer_token_redaction() {
        let detector = DlpDetector::new();
        let content = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9_longtokenvalue";
        
        let result = detector.scan(content);
        
        assert!(result.has_sensitive_data);
        assert!(!result.should_block); // Bearer tokens are redacted, not blocked
        
        let bearer_match = result.matches.iter()
            .find(|m| m.pattern_name == "bearer_token");
        assert!(bearer_match.is_some());
        assert_eq!(bearer_match.unwrap().action, DlpAction::Redact);
    }

    #[test]
    fn test_scan_and_clean_blocks_critical_secrets() {
        let detector = DlpDetector::new();
        let content = "API key: sk-proj-test1234567890abcdefghij";
        
        let result = detector.scan_and_clean(content);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("blocked"));
    }

    #[test]
    fn test_scan_and_clean_passes_clean_content() {
        let detector = DlpDetector::new();
        let content = "This is just regular text with no secrets.";
        
        let result = detector.scan_and_clean(content);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_scan_http_request_clean() {
        let detector = DlpDetector::new();
        
        let result = detector.scan_http_request(
            "https://api.example.com/data",
            &[("Content-Type".to_string(), "application/json".to_string())],
            Some(b"{\"query\": \"hello\"}"),
        );
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_scan_http_request_blocks_secret_in_url() {
        let detector = DlpDetector::new();
        
        let result = detector.scan_http_request(
            "https://evil.com/steal?key=AKIAIOSFODNN7EXAMPLE",
            &[],
            None,
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_http_request_blocks_secret_in_header() {
        let detector = DlpDetector::new();
        
        let result = detector.scan_http_request(
            "https://api.example.com/data",
            &[(
                "X-Custom".to_string(),
                "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
            )],
            None,
        );
        
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_secrets_detection() {
        let detector = DlpDetector::new();
        let content = "AWS: AKIAIOSFODNN7EXAMPLE and GitHub: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        
        let result = detector.scan(content);
        
        assert!(result.has_sensitive_data);
        assert!(result.should_block);
        assert!(result.matches.len() >= 2);
        assert_eq!(result.max_severity, Some(DlpSeverity::Critical));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(DlpSeverity::Critical > DlpSeverity::High);
        assert!(DlpSeverity::High > DlpSeverity::Medium);
        assert!(DlpSeverity::Medium > DlpSeverity::Low);
    }
}