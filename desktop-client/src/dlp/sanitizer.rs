//! DLP 脱敏处理器
//!
//! 提供灵活的脱敏配置和处理功能

use crate::dlp::{DlpAction, DlpDetectionResult, DlpDetector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

/// 脱敏配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizationConfig {
    /// 是否启用脱敏
    pub enabled: bool,
    /// 脱敏替换映射
    pub replacement_map: HashMap<String, String>,
    /// 默认脱敏文本
    pub default_redaction: String,
    /// 是否保留格式（如保留身份证的前后几位）
    pub preserve_format: bool,
    /// 格式保留配置
    pub format_preservation: FormatPreservationConfig,
}

/// 格式保留配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatPreservationConfig {
    /// 身份证号保留前后位数
    pub id_card_preserve_chars: usize,
    /// 手机号保留前后位数
    pub mobile_preserve_chars: usize,
    /// 银行卡号保留前后位数
    pub bank_card_preserve_chars: usize,
    /// 脱敏字符
    pub mask_char: char,
}

impl Default for SanitizationConfig {
    fn default() -> Self {
        let mut replacement_map = HashMap::new();
        replacement_map.insert("chinese_id_card_18".to_string(), "[身份证号]".to_string());
        replacement_map.insert("chinese_id_card_15".to_string(), "[身份证号]".to_string());
        replacement_map.insert("chinese_mobile".to_string(), "[手机号]".to_string());
        replacement_map.insert("chinese_bank_card".to_string(), "[银行卡号]".to_string());
        replacement_map.insert("aliyun_access_key".to_string(), "[阿里云密钥]".to_string());
        replacement_map.insert(
            "tencent_cloud_secret_id".to_string(),
            "[腾讯云密钥]".to_string(),
        );

        Self {
            enabled: true,
            replacement_map,
            default_redaction: "[敏感信息]".to_string(),
            preserve_format: true,
            format_preservation: FormatPreservationConfig {
                id_card_preserve_chars: 3,
                mobile_preserve_chars: 3,
                bank_card_preserve_chars: 4,
                mask_char: '*',
            },
        }
    }
}

/// 脱敏结果
#[derive(Debug, Serialize, Deserialize)]
pub struct SanitizationResult {
    /// 原始内容是否包含敏感信息
    pub had_sensitive_data: bool,
    /// 脱敏后的内容
    pub sanitized_content: String,
    /// 是否被阻止（包含不可脱敏的敏感信息）
    pub was_blocked: bool,
    /// 阻止原因
    pub block_reason: Option<String>,
    /// 脱敏统计
    pub sanitization_stats: SanitizationStats,
}

/// 脱敏统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizationStats {
    /// 检测到的敏感信息总数
    pub total_matches: usize,
    /// 脱敏处理的数量
    pub redacted_count: usize,
    /// 警告的数量
    pub warned_count: usize,
    /// 阻止的数量
    pub blocked_count: usize,
    /// 按严重程度分组的统计
    pub severity_stats: HashMap<String, usize>,
}

impl Default for SanitizationStats {
    fn default() -> Self {
        Self {
            total_matches: 0,
            redacted_count: 0,
            warned_count: 0,
            blocked_count: 0,
            severity_stats: HashMap::new(),
        }
    }
}

/// DLP 脱敏处理器
pub struct DlpSanitizer {
    detector: DlpDetector,
    config: SanitizationConfig,
}

impl DlpSanitizer {
    /// 创建新的脱敏处理器
    pub fn new(detector: DlpDetector, config: SanitizationConfig) -> Self {
        Self { detector, config }
    }

    /// 使用默认配置创建脱敏处理器
    pub fn with_default_config(detector: DlpDetector) -> Self {
        Self::new(detector, SanitizationConfig::default())
    }

    /// 更新脱敏配置
    pub fn update_config(&mut self, config: SanitizationConfig) {
        self.config = config;
    }

    /// 获取当前配置
    pub fn config(&self) -> &SanitizationConfig {
        &self.config
    }

    /// 获取检测器引用
    pub fn detector(&self) -> &DlpDetector {
        &self.detector
    }

    /// 脱敏处理文本
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub fn sanitize(&self, content: &str) -> SanitizationResult {
        debug!("Starting sanitization process");

        if !self.config.enabled {
            debug!("Sanitization disabled, returning original content");
            return SanitizationResult {
                had_sensitive_data: false,
                sanitized_content: content.to_string(),
                was_blocked: false,
                block_reason: None,
                sanitization_stats: SanitizationStats::default(),
            };
        }

        let detection_result = self.detector.scan(content);

        if !detection_result.has_sensitive_data {
            debug!("No sensitive data detected");
            return SanitizationResult {
                had_sensitive_data: false,
                sanitized_content: content.to_string(),
                was_blocked: false,
                block_reason: None,
                sanitization_stats: SanitizationStats::default(),
            };
        }

        // 检查是否有需要阻止的内容
        if detection_result.should_block {
            let block_patterns: Vec<String> = detection_result
                .matches
                .iter()
                .filter(|m| m.action == DlpAction::Block)
                .map(|m| m.pattern_name.clone())
                .collect();

            warn!(
                patterns = ?block_patterns,
                "Content blocked due to critical sensitive data"
            );

            return SanitizationResult {
                had_sensitive_data: true,
                sanitized_content: String::new(),
                was_blocked: true,
                block_reason: Some(format!(
                    "包含不可脱敏的敏感信息: {}",
                    block_patterns.join(", ")
                )),
                sanitization_stats: self.calculate_stats(&detection_result),
            };
        }

        // 执行脱敏处理
        let sanitized_content = self.apply_sanitization(content, &detection_result);

        debug!(
            matches_count = detection_result.matches.len(),
            "Sanitization completed"
        );

        SanitizationResult {
            had_sensitive_data: true,
            sanitized_content,
            was_blocked: false,
            block_reason: None,
            sanitization_stats: self.calculate_stats(&detection_result),
        }
    }

    /// 应用脱敏处理
    fn apply_sanitization(&self, content: &str, detection_result: &DlpDetectionResult) -> String {
        // 检查是否有自定义替换需求（replacement_map 中有匹配的 pattern_name，
        // 或者 default_redaction 不是 LeakDetector 的默认值 "[REDACTED]"）
        let has_custom_replacements = detection_result
            .matches
            .iter()
            .any(|m| self.config.replacement_map.contains_key(&m.pattern_name))
            || self.config.default_redaction != "[REDACTED]";

        if let Some(ref already_redacted) = detection_result.sanitized_content {
            // LeakDetector 已经做了脱敏，判断是否需要重新处理
            let needs_format_preservation = self.config.preserve_format
                && detection_result.matches.iter().any(|m| {
                    matches!(
                        m.pattern_name.as_str(),
                        "chinese_id_card_18"
                            | "chinese_id_card_15"
                            | "chinese_mobile"
                            | "chinese_bank_card"
                    )
                });

            if needs_format_preservation || has_custom_replacements {
                // 用自定义替换/格式保留重新处理原始内容
                return self.apply_format_preserving_replacement(content, detection_result);
            }

            return already_redacted.clone();
        }

        // 回退：手动替换（字节边界安全版本）
        self.apply_format_preserving_replacement(content, detection_result)
    }

    /// 格式保留替换（字节边界安全）
    fn apply_format_preserving_replacement(
        &self,
        content: &str,
        detection_result: &DlpDetectionResult,
    ) -> String {
        // 转为 char 数组操作，避免字节偏移问题
        let chars: Vec<char> = content.chars().collect();

        // 构建字节偏移 → char 索引的映射
        let mut byte_to_char: Vec<usize> = vec![0; content.len() + 1];
        let mut char_idx = 0;
        for (byte_idx, _ch) in content.char_indices() {
            byte_to_char[byte_idx] = char_idx;
            char_idx += 1;
        }
        byte_to_char[content.len()] = char_idx;

        // 按 char 位置倒序排序，避免替换时偏移
        let mut replacements: Vec<(usize, usize, String)> = Vec::new();
        for dlp_match in &detection_result.matches {
            if dlp_match.action != DlpAction::Redact {
                continue;
            }
            let start_byte = dlp_match.location.start;
            let end_byte = dlp_match.location.end;
            if end_byte > content.len() {
                continue;
            }
            let start_char = byte_to_char[start_byte];
            let end_char = byte_to_char[end_byte];
            let original: String = chars[start_char..end_char].iter().collect();
            let replacement = self.get_replacement_for_match(dlp_match, &original);
            replacements.push((start_char, end_char, replacement));
        }

        // 倒序替换
        replacements.sort_by(|a, b| b.0.cmp(&a.0));
        let mut result_chars = chars;
        for (start_char, end_char, replacement) in replacements {
            let replacement_chars: Vec<char> = replacement.chars().collect();
            result_chars.splice(start_char..end_char, replacement_chars);
        }

        result_chars.iter().collect()
    }

    /// 根据匹配获取替换文本
    fn get_replacement_for_match(
        &self,
        dlp_match: &crate::dlp::DlpMatch,
        original_text: &str,
    ) -> String {
        if let Some(replacement) = self.config.replacement_map.get(&dlp_match.pattern_name) {
            if self.config.preserve_format {
                return self.apply_format_preservation_for_text(
                    dlp_match,
                    original_text,
                    replacement,
                );
            }
            return replacement.clone();
        }
        if self.config.preserve_format {
            self.apply_format_preservation_for_text(
                dlp_match,
                original_text,
                &self.config.default_redaction,
            )
        } else {
            self.config.default_redaction.clone()
        }
    }

    /// 对已提取的原始文本应用格式保留
    fn apply_format_preservation_for_text(
        &self,
        dlp_match: &crate::dlp::DlpMatch,
        original_text: &str,
        fallback_replacement: &str,
    ) -> String {
        let preserve_config = &self.config.format_preservation;
        match dlp_match.pattern_name.as_str() {
            "chinese_id_card_18" | "chinese_id_card_15" => self.preserve_format_generic(
                original_text,
                preserve_config.id_card_preserve_chars,
                preserve_config.mask_char,
            ),
            "chinese_mobile" => self.preserve_format_mobile(
                original_text,
                preserve_config.mobile_preserve_chars,
                preserve_config.mask_char,
            ),
            "chinese_bank_card" => self.preserve_format_generic(
                original_text,
                preserve_config.bank_card_preserve_chars,
                preserve_config.mask_char,
            ),
            _ => fallback_replacement.to_string(),
        }
    }

    /// 通用格式保留
    fn preserve_format_generic(
        &self,
        text: &str,
        preserve_chars: usize,
        mask_char: char,
    ) -> String {
        let len = text.len();
        if len <= preserve_chars * 2 {
            return mask_char.to_string().repeat(len);
        }

        let prefix = &text[..preserve_chars];
        let suffix = &text[len - preserve_chars..];
        let middle_len = len - preserve_chars * 2;

        format!(
            "{}{}{}",
            prefix,
            mask_char.to_string().repeat(middle_len),
            suffix
        )
    }

    /// 手机号格式保留（处理国际区号）
    fn preserve_format_mobile(&self, text: &str, preserve_chars: usize, mask_char: char) -> String {
        // 如果包含+86等国际区号，保留区号部分
        if let Some(rest) = text.strip_prefix("+86") {
            let mobile_part = rest.trim_start_matches(&['-', ' '][..]);
            let preserved_mobile =
                self.preserve_format_generic(mobile_part, preserve_chars, mask_char);
            // 保持原有的分隔符
            if text.contains(" ") {
                format!("+86 {}", preserved_mobile)
            } else if text.contains("-") {
                format!("+86-{}", preserved_mobile)
            } else {
                format!("+86{}", preserved_mobile)
            }
        } else {
            self.preserve_format_generic(text.trim(), preserve_chars, mask_char)
        }
    }

    /// 计算脱敏统计
    fn calculate_stats(&self, detection_result: &DlpDetectionResult) -> SanitizationStats {
        let mut stats = SanitizationStats {
            total_matches: detection_result.matches.len(),
            ..Default::default()
        };

        for dlp_match in &detection_result.matches {
            match dlp_match.action {
                DlpAction::Warn => stats.warned_count += 1,
                DlpAction::Redact => stats.redacted_count += 1,
                DlpAction::Block => stats.blocked_count += 1,
            }

            let severity_key = format!("{:?}", dlp_match.severity);
            *stats.severity_stats.entry(severity_key).or_insert(0) += 1;
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dlp::patterns::get_all_builtin_patterns;

    fn create_test_sanitizer() -> DlpSanitizer {
        let patterns = get_all_builtin_patterns().unwrap();
        let detector = DlpDetector::with_custom_patterns(patterns);
        DlpSanitizer::with_default_config(detector)
    }

    #[test]
    fn test_sanitizer_creation() {
        let sanitizer = create_test_sanitizer();
        assert!(sanitizer.config().enabled);
        assert!(!sanitizer.config().replacement_map.is_empty());
    }

    #[test]
    fn test_sanitize_clean_content() {
        let sanitizer = create_test_sanitizer();
        let content = "这是一段没有敏感信息的普通文本。";

        let result = sanitizer.sanitize(content);

        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.sanitized_content, content);
        assert_eq!(result.sanitization_stats.total_matches, 0);
    }

    #[test]
    fn test_sanitize_chinese_id_card() {
        let sanitizer = create_test_sanitizer();
        let content = "我的身份证号是 110101199003071234 ，请保密。";

        let result = sanitizer.sanitize(content);

        println!("Original content: {}", content);
        println!("Sanitized content: {}", result.sanitized_content);
        println!("Had sensitive data: {}", result.had_sensitive_data);
        println!("Was blocked: {}", result.was_blocked);
        println!("Matches: {:?}", result.sanitization_stats);

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("110************234")); // 格式保留
        assert_eq!(result.sanitization_stats.redacted_count, 1);
    }

    #[test]
    fn test_sanitize_chinese_mobile() {
        let sanitizer = create_test_sanitizer();
        let content = "联系电话： 13800138000 ";

        let result = sanitizer.sanitize(content);

        println!("Mobile content: {}", content);
        println!("Mobile sanitized: {}", result.sanitized_content);
        println!(
            "Mobile matches: {}",
            result.sanitization_stats.total_matches
        );
        println!(
            "Mobile redacted: {}",
            result.sanitization_stats.redacted_count
        );

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("138*****000")); // 格式保留
        assert_eq!(result.sanitization_stats.redacted_count, 1);
    }

    #[test]
    fn test_sanitize_mobile_with_country_code() {
        let sanitizer = create_test_sanitizer();
        let content = "国际号码： +86 13800138000 ";

        let result = sanitizer.sanitize(content);

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("+86 138*****000")); // 保留国际区号
    }

    #[test]
    fn test_sanitize_api_key_blocked() {
        let sanitizer = create_test_sanitizer();
        let content = "阿里云密钥： LTAI4G8aB9cD2eFgH3iJ ";

        let result = sanitizer.sanitize(content);

        assert!(result.had_sensitive_data);
        assert!(result.was_blocked);
        assert!(result.block_reason.is_some());
        assert!(result.block_reason.unwrap().contains("aliyun_access_key"));
        assert_eq!(result.sanitization_stats.blocked_count, 1);
    }

    #[test]
    fn test_sanitize_multiple_sensitive_data() {
        let sanitizer = create_test_sanitizer();
        let content = "用户信息：身份证 110101199003071234 ，手机 13800138000 ";

        let result = sanitizer.sanitize(content);

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.sanitization_stats.total_matches, 2);
        assert_eq!(result.sanitization_stats.redacted_count, 2);

        // 检查两个敏感信息都被脱敏
        assert!(result.sanitized_content.contains("110************234"));
        assert!(result.sanitized_content.contains("138*****000"));
    }

    #[test]
    fn test_sanitize_disabled() {
        let patterns = get_all_builtin_patterns().unwrap();
        let detector = DlpDetector::with_custom_patterns(patterns);
        let config = SanitizationConfig {
            enabled: false,
            ..Default::default()
        };
        let sanitizer = DlpSanitizer::new(detector, config);

        let content = "身份证：110101199003071234";
        let result = sanitizer.sanitize(content);

        assert!(!result.had_sensitive_data);
        assert_eq!(result.sanitized_content, content); // 原样返回
    }

    #[test]
    fn test_custom_replacement_map() {
        let patterns = get_all_builtin_patterns().unwrap();
        let detector = DlpDetector::with_custom_patterns(patterns);
        let mut config = SanitizationConfig::default();
        config.replacement_map.insert(
            "chinese_id_card_18".to_string(),
            "[自定义身份证脱敏]".to_string(),
        );
        config.preserve_format = false; // 禁用格式保留以测试自定义替换
        let sanitizer = DlpSanitizer::new(detector, config);

        let content = "身份证：110101199003071234";
        let result = sanitizer.sanitize(content);

        assert!(result.had_sensitive_data);
        assert!(result.sanitized_content.contains("[自定义身份证脱敏]"));
    }

    #[test]
    fn test_format_preservation_config() {
        let patterns = get_all_builtin_patterns().unwrap();
        let detector = DlpDetector::with_custom_patterns(patterns);
        let mut config = SanitizationConfig::default();
        config.format_preservation.id_card_preserve_chars = 2; // 只保留前后2位
        config.format_preservation.mask_char = '#';
        let sanitizer = DlpSanitizer::new(detector, config);

        let content = "身份证：110101199003071234";
        let result = sanitizer.sanitize(content);

        assert!(result.had_sensitive_data);
        assert!(result.sanitized_content.contains("11##############34"));
    }

    #[test]
    fn test_preserve_format_generic() {
        let sanitizer = create_test_sanitizer();

        // 测试通用格式保留
        let result = sanitizer.preserve_format_generic("1234567890", 2, '*');
        assert_eq!(result, "12******90");

        // 测试短文本
        let result = sanitizer.preserve_format_generic("123", 2, '*');
        assert_eq!(result, "***");
    }

    #[test]
    fn test_preserve_format_mobile() {
        let sanitizer = create_test_sanitizer();

        // 测试普通手机号
        let result = sanitizer.preserve_format_mobile("13800138000", 3, '*');
        assert_eq!(result, "138*****000");

        // 测试带国际区号的手机号
        let result = sanitizer.preserve_format_mobile("+8613800138000", 3, '*');
        assert_eq!(result, "+86138*****000");

        // 测试带空格的国际区号
        let result = sanitizer.preserve_format_mobile("+86 13800138000", 3, '*');
        assert_eq!(result, "+86 138*****000");
    }

    #[test]
    fn test_sanitization_stats() {
        let sanitizer = create_test_sanitizer();
        let content =
            "身份证： 110101199003071234 ，手机： 13800138000 ，阿里云密钥： LTAI4G8aB9cD2eFgH3iJ ";

        let result = sanitizer.sanitize(content);

        assert!(result.was_blocked); // 因为包含API密钥
        assert_eq!(result.sanitization_stats.total_matches, 3);
        assert_eq!(result.sanitization_stats.redacted_count, 2); // 身份证和手机号
        assert_eq!(result.sanitization_stats.blocked_count, 1); // API密钥

        // 检查严重程度统计
        assert!(result
            .sanitization_stats
            .severity_stats
            .contains_key("High"));
        assert!(result
            .sanitization_stats
            .severity_stats
            .contains_key("Medium"));
        assert!(result
            .sanitization_stats
            .severity_stats
            .contains_key("Critical"));
    }

    #[test]
    fn test_update_config() {
        let mut sanitizer = create_test_sanitizer();

        let new_config = SanitizationConfig {
            enabled: false,
            ..Default::default()
        };

        sanitizer.update_config(new_config);

        assert!(!sanitizer.config().enabled);
    }
}
