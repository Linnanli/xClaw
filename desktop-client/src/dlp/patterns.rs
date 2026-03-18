//! DLP 检测模式定义
//!
//! 包含中国特色敏感信息检测模式和API密钥检测模式

use ironclaw_safety::{LeakPattern, LeakSeverity, LeakAction};
use regex::Regex;
use crate::dlp::{DlpError, DlpResult};

/// 中国特色敏感信息检测模式
pub struct ChinesePatterns;

impl ChinesePatterns {
    /// 获取中国身份证号码检测模式
    pub fn chinese_id_card_patterns() -> DlpResult<Vec<LeakPattern>> {
        let patterns = vec![
            // 18位身份证号码（使用单词边界）
            LeakPattern {
                name: "chinese_id_card_18".to_string(),
                regex: Regex::new(r"\b[1-9]\d{5}(19|20)\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}[\dXx]\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::High,
                action: LeakAction::Redact,
            },
            // 15位身份证号码（旧版）
            LeakPattern {
                name: "chinese_id_card_15".to_string(),
                regex: Regex::new(r"\b[1-9]\d{7}(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])\d{3}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::High,
                action: LeakAction::Redact,
            },
        ];
        Ok(patterns)
    }

    /// 获取中国手机号码检测模式
    pub fn chinese_mobile_patterns() -> DlpResult<Vec<LeakPattern>> {
        let patterns = vec![
            // 带国际区号的中国手机号（优先匹配，包含不带区号的情况）
            LeakPattern {
                name: "chinese_mobile".to_string(),
                regex: Regex::new(r"(\+86[-\s]?)?\b1[3-9]\d{9}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::Medium,
                action: LeakAction::Redact,
            },
        ];
        Ok(patterns)
    }

    /// 获取中国银行卡号检测模式
    pub fn chinese_bank_card_patterns() -> DlpResult<Vec<LeakPattern>> {
        let patterns = vec![
            // 中国银行卡号（16-19位）
            LeakPattern {
                name: "chinese_bank_card".to_string(),
                regex: Regex::new(r"[4-6]\d{15,18}")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::High,
                action: LeakAction::Redact,
            },
        ];
        Ok(patterns)
    }

    /// 获取所有中国特色敏感信息检测模式
    pub fn all_chinese_patterns() -> DlpResult<Vec<LeakPattern>> {
        let mut patterns = Vec::new();
        patterns.extend(Self::chinese_id_card_patterns()?);
        patterns.extend(Self::chinese_mobile_patterns()?);
        patterns.extend(Self::chinese_bank_card_patterns()?);
        Ok(patterns)
    }
}

/// API密钥检测模式
pub struct ApiKeyPatterns;

impl ApiKeyPatterns {
    /// 获取中国云服务商API密钥检测模式
    pub fn chinese_cloud_api_patterns() -> DlpResult<Vec<LeakPattern>> {
        let patterns = vec![
            // 阿里云 AccessKey
            LeakPattern {
                name: "aliyun_access_key".to_string(),
                regex: Regex::new(r"\bLTAI[a-zA-Z0-9]{12,20}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::Critical,
                action: LeakAction::Block,
            },
            // 腾讯云 SecretId
            LeakPattern {
                name: "tencent_cloud_secret_id".to_string(),
                regex: Regex::new(r"\bAKID[a-zA-Z0-9]{32}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::Critical,
                action: LeakAction::Block,
            },
            // 华为云 Access Key
            LeakPattern {
                name: "huawei_cloud_access_key".to_string(),
                regex: Regex::new(r"\b[A-Z0-9]{20}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::High,
                action: LeakAction::Warn, // 较宽泛的模式，仅警告
            },
        ];
        Ok(patterns)
    }

    /// 获取国产AI服务API密钥检测模式
    pub fn chinese_ai_api_patterns() -> DlpResult<Vec<LeakPattern>> {
        let patterns = vec![
            // 百度文心一言 API Key
            LeakPattern {
                name: "baidu_ernie_api_key".to_string(),
                regex: Regex::new(r"\b[a-zA-Z0-9]{24}\.ERNIE-[a-zA-Z0-9-]+\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::Critical,
                action: LeakAction::Block,
            },
            // 阿里云通义千问 API Key
            LeakPattern {
                name: "aliyun_qwen_api_key".to_string(),
                regex: Regex::new(r"\bsk-[a-zA-Z0-9]{32,}\b")
                    .map_err(|e| DlpError::PatternCompilation(e.to_string()))?,
                severity: LeakSeverity::Critical,
                action: LeakAction::Block,
            },
        ];
        Ok(patterns)
    }

    /// 获取所有API密钥检测模式
    pub fn all_api_key_patterns() -> DlpResult<Vec<LeakPattern>> {
        let mut patterns = Vec::new();
        patterns.extend(Self::chinese_cloud_api_patterns()?);
        patterns.extend(Self::chinese_ai_api_patterns()?);
        Ok(patterns)
    }
}

/// 自定义检测模式
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub severity: LeakSeverity,
    pub action: LeakAction,
    pub description: Option<String>,
}

impl CustomPattern {
    /// 创建新的自定义模式
    pub fn new(
        name: String,
        pattern: String,
        severity: LeakSeverity,
        action: LeakAction,
    ) -> Self {
        Self {
            name,
            pattern,
            severity,
            action,
            description: None,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 转换为 LeakPattern
    pub fn to_leak_pattern(self) -> DlpResult<LeakPattern> {
        let regex = Regex::new(&self.pattern)
            .map_err(|e| DlpError::PatternCompilation(format!("Pattern '{}': {}", self.name, e)))?;
        
        Ok(LeakPattern {
            name: self.name,
            regex,
            severity: self.severity,
            action: self.action,
        })
    }
}

/// 获取所有内置检测模式
pub fn get_all_builtin_patterns() -> DlpResult<Vec<LeakPattern>> {
    let mut patterns = Vec::new();
    
    // 添加中国特色模式
    patterns.extend(ChinesePatterns::all_chinese_patterns()?);
    
    // 添加API密钥模式
    patterns.extend(ApiKeyPatterns::all_api_key_patterns()?);
    
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chinese_id_card_18_digit_pattern() {
        let patterns = ChinesePatterns::chinese_id_card_patterns().unwrap();
        let pattern = &patterns[0]; // 18位身份证
        
        // 有效的18位身份证号码
        let valid_ids = [
            "110101199003071234",
            "320106198506234567",
            "440301199912315678",
        ];
        
        for id in &valid_ids {
            assert!(
                pattern.regex.is_match(id),
                "Valid 18-digit ID {} should match",
                id
            );
        }
        
        // 在中文句子中的身份证号码
        let chinese_content = "我的身份证号是 110101199003071234 ，请保密。";
        assert!(
            pattern.regex.is_match(chinese_content),
            "ID in Chinese sentence should match"
        );
        
        // 无效的身份证号码
        let invalid_ids = [
            "000000199003071234", // 地区码无效
            "110101199013071234", // 月份无效
            "110101199003321234", // 日期无效
            "11010119900307123",  // 位数不足
        ];
        
        for id in &invalid_ids {
            // 注意：移除单词边界后，某些无效ID可能仍然匹配部分内容
            // 这里我们主要测试完整的无效ID不应该匹配
            let full_match = pattern.regex.find(id)
                .map(|m| m.as_str() == *id)
                .unwrap_or(false);
            assert!(
                !full_match,
                "Invalid ID {} should not fully match",
                id
            );
        }
    }

    #[test]
    fn test_chinese_id_card_15_digit_pattern() {
        let patterns = ChinesePatterns::chinese_id_card_patterns().unwrap();
        let pattern = &patterns[1]; // 15位身份证
        
        // 有效的15位身份证号码
        let valid_ids = [
            "110101901231123",
            "320106850623456",
        ];
        
        for id in &valid_ids {
            assert!(
                pattern.regex.is_match(id),
                "Valid 15-digit ID {} should match",
                id
            );
        }
    }

    #[test]
    fn test_chinese_mobile_pattern() {
        let patterns = ChinesePatterns::chinese_mobile_patterns().unwrap();
        let pattern = &patterns[0]; // 中国手机号
        
        // 有效的手机号码
        let valid_mobiles = [
            "13800138000",
            "15912345678",
            "18612345678",
            "19912345678",
        ];
        
        for mobile in &valid_mobiles {
            assert!(
                pattern.regex.is_match(mobile),
                "Valid mobile {} should match",
                mobile
            );
        }
        
        // 无效的手机号码
        let invalid_mobiles = [
            "12812345678", // 不是1[3-9]开头
            "1381234567",  // 位数不足
            "138123456789", // 位数过多
        ];
        
        for mobile in &invalid_mobiles {
            // 测试完整匹配，不应该匹配整个字符串
            let full_match = pattern.regex.find(mobile)
                .map(|m| m.as_str() == *mobile)
                .unwrap_or(false);
            assert!(
                !full_match,
                "Invalid mobile {} should not fully match",
                mobile
            );
        }
    }

    #[test]
    fn test_chinese_mobile_with_country_code_pattern() {
        let patterns = ChinesePatterns::chinese_mobile_patterns().unwrap();
        let pattern = &patterns[0]; // 带国际区号的手机号
        
        // 有效的带区号手机号码
        let valid_mobiles = [
            "+86 13800138000",
            "+86-13800138000",
            " 13800138000 ", // 不带区号也应该匹配
        ];
        
        for mobile in &valid_mobiles {
            assert!(
                pattern.regex.is_match(mobile),
                "Valid mobile with country code {} should match",
                mobile
            );
        }
    }

    #[test]
    fn test_aliyun_access_key_pattern() {
        let patterns = ApiKeyPatterns::chinese_cloud_api_patterns().unwrap();
        let aliyun_pattern = patterns.iter()
            .find(|p| p.name == "aliyun_access_key")
            .unwrap();
        
        // 有效的阿里云AccessKey
        let valid_keys = [
            "LTAI4G8aB9cD2eFgH3iJ",
            "LTAI5K6mN7oP8qR9sT0u",
        ];
        
        for key in &valid_keys {
            assert!(
                aliyun_pattern.regex.is_match(key),
                "Valid Aliyun AccessKey {} should match",
                key
            );
        }
        
        // 无效的AccessKey
        let invalid_keys = [
            "LTAI123", // 太短
            "XTAI4G8aB9cD2eFgH3iJ", // 不是LTAI开头
        ];
        
        for key in &invalid_keys {
            assert!(
                !aliyun_pattern.regex.is_match(key),
                "Invalid AccessKey {} should not match",
                key
            );
        }
    }

    #[test]
    fn test_tencent_cloud_secret_id_pattern() {
        let patterns = ApiKeyPatterns::chinese_cloud_api_patterns().unwrap();
        let tencent_pattern = patterns.iter()
            .find(|p| p.name == "tencent_cloud_secret_id")
            .unwrap();
        
        // 有效的腾讯云SecretId
        let valid_ids = [
            "AKIDabcdefghijklmnopqrstuvwxyz123456",
        ];
        
        for id in &valid_ids {
            assert!(
                tencent_pattern.regex.is_match(id),
                "Valid Tencent Cloud SecretId {} should match",
                id
            );
        }
    }

    #[test]
    fn test_custom_pattern_creation() {
        let custom = CustomPattern::new(
            "test_pattern".to_string(),
            r"\bTEST-\d{4}\b".to_string(),
            LeakSeverity::Medium,
            LeakAction::Redact,
        ).with_description("Test pattern for unit tests".to_string());
        
        let leak_pattern = custom.to_leak_pattern().unwrap();
        
        assert_eq!(leak_pattern.name, "test_pattern");
        assert_eq!(leak_pattern.severity, LeakSeverity::Medium);
        assert_eq!(leak_pattern.action, LeakAction::Redact);
        
        // 测试模式匹配
        assert!(leak_pattern.regex.is_match("TEST-1234"));
        assert!(!leak_pattern.regex.is_match("TEST-123"));
    }

    #[test]
    fn test_custom_pattern_invalid_regex() {
        let custom = CustomPattern::new(
            "invalid_pattern".to_string(),
            r"[invalid regex(".to_string(), // 无效的正则表达式
            LeakSeverity::Low,
            LeakAction::Warn,
        );
        
        let result = custom.to_leak_pattern();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Pattern 'invalid_pattern'"));
    }

    #[test]
    fn test_get_all_builtin_patterns() {
        let patterns = get_all_builtin_patterns().unwrap();
        
        // 应该包含中国身份证、手机号、银行卡和API密钥模式
        assert!(patterns.len() > 5);
        
        // 检查是否包含关键模式
        let pattern_names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
        assert!(pattern_names.contains(&"chinese_id_card_18"));
        assert!(pattern_names.contains(&"chinese_mobile"));
        assert!(pattern_names.contains(&"aliyun_access_key"));
    }

    #[test]
    fn test_pattern_severity_and_action_assignment() {
        let chinese_patterns = ChinesePatterns::all_chinese_patterns().unwrap();
        let api_patterns = ApiKeyPatterns::all_api_key_patterns().unwrap();
        
        // 身份证和手机号应该是脱敏处理
        for pattern in &chinese_patterns {
            if pattern.name.contains("id_card") || pattern.name.contains("mobile") {
                assert_eq!(pattern.action, LeakAction::Redact);
            }
        }
        
        // API密钥应该是阻止处理（除了华为云的宽泛模式）
        for pattern in &api_patterns {
            if pattern.name != "huawei_cloud_access_key" {
                assert_eq!(pattern.action, LeakAction::Block);
                assert!(pattern.severity >= LeakSeverity::Critical || pattern.severity >= LeakSeverity::High);
            }
        }
    }
}