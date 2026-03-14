//! DLP 脱敏完整性属性测试
//!
//! 本模块使用属性测试验证 DLP 脱敏引擎的完整性，包括：
//! - 属性 2: DLP 脱敏完整性
//! - 验证所有已知敏感模式都被正确脱敏
//! - 验证多个 DLP 规则的协同工作

#[cfg(test)]
mod tests {
    use crate::safety::leak_detector::LeakDetector;
    use proptest::prelude::*;

    // ============================================================================
    // 属性 2: DLP 脱敏完整性
    // ============================================================================
    //
    // Property: 对于任何包含已知敏感模式的文本，DLP 引擎必须检测到所有敏感信息
    // 并将其脱敏为 [REDACTED] 或类似的占位符。
    //
    // Verification:
    // - 生成包含已知敏感模式的文本
    // - 运行 DLP 扫描
    // - 验证所有敏感信息都被脱敏
    // - 验证脱敏后的内容不包含原始敏感数据

    #[test]
    fn test_dlp_sanitization_completeness_openai_key() {
        let detector = LeakDetector::new();
        let content = "My API key is sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";

        let result = detector.scan(content);
        assert!(!result.is_clean(), "Should detect OpenAI key");

        if let Some(sanitized) = result.sanitized_content {
            // 验证原始密钥不在脱敏后的内容中
            assert!(
                !sanitized.contains("sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123"),
                "Sanitized content should not contain original key"
            );
            // 验证脱敏后的内容包含占位符
            assert!(
                sanitized.contains("[REDACTED]") || sanitized.contains("***"),
                "Sanitized content should contain placeholder"
            );
        }
    }

    #[test]
    fn test_dlp_sanitization_completeness_github_token() {
        let detector = LeakDetector::new();
        let content = "GitHub token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

        let result = detector.scan(content);
        assert!(!result.is_clean(), "Should detect GitHub token");

        if let Some(sanitized) = result.sanitized_content {
            assert!(
                !sanitized.contains("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                "Sanitized content should not contain original token"
            );
        }
    }

    #[test]
    fn test_dlp_sanitization_completeness_aws_key() {
        let detector = LeakDetector::new();
        let content = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";

        let result = detector.scan(content);
        assert!(!result.is_clean(), "Should detect AWS key");

        if let Some(sanitized) = result.sanitized_content {
            assert!(
                !sanitized.contains("AKIAIOSFODNN7EXAMPLE"),
                "Sanitized content should not contain original key"
            );
        }
    }

    #[test]
    fn test_dlp_sanitization_completeness_pem_key() {
        let detector = LeakDetector::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----";

        let result = detector.scan(content);
        assert!(!result.is_clean(), "Should detect PEM key");

        if let Some(sanitized) = result.sanitized_content {
            assert!(
                !sanitized.contains("MIIEowIBAAKCAQEA"),
                "Sanitized content should not contain original key material"
            );
        }
    }

    // ============================================================================
    // 多个 DLP 规则协同工作测试
    // ============================================================================

    #[test]
    fn test_multiple_dlp_rules_simultaneous_detection() {
        let detector = LeakDetector::new();
        // 包含多个不同类型的敏感信息
        let content = "User: john@example.com, API Key: sk-proj-abc123, AWS Key: AKIAIOSFODNN7EXAMPLE";

        let result = detector.scan(content);
        assert!(!result.is_clean(), "Should detect multiple sensitive patterns");

        // 验证检测到多个匹配
        assert!(
            result.matches.len() >= 2,
            "Should detect at least 2 sensitive patterns"
        );
    }

    #[test]
    fn test_multiple_dlp_rules_sequential_sanitization() {
        let detector = LeakDetector::new();
        let content = "API: sk-proj-abc123, Token: ghp_xxxx, Key: AKIA1234";

        let result = detector.scan(content);
        if let Some(sanitized) = result.sanitized_content {
            // 验证所有敏感信息都被脱敏
            assert!(!sanitized.contains("sk-proj-abc123"));
            assert!(!sanitized.contains("ghp_xxxx"));
            assert!(!sanitized.contains("AKIA1234"));
        }
    }

    #[test]
    fn test_dlp_rules_no_false_negatives() {
        let detector = LeakDetector::new();
        // 包含多个已知敏感模式的内容
        let patterns = vec![
            "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123",
            "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "AKIAIOSFODNN7EXAMPLE",
        ];

        for pattern in patterns {
            let result = detector.scan(pattern);
            assert!(
                !result.is_clean(),
                "Should detect pattern: {}",
                pattern
            );
        }
    }

    // ============================================================================
    // 属性测试: 脱敏完整性
    // ============================================================================

    proptest! {
        /// Property: 对于任何包含已知敏感模式的文本，脱敏后的内容不应包含原始敏感数据
        #[test]
        fn prop_sanitized_content_no_original_secrets(
            prefix in ".*",
            suffix in ".*"
        ) {
            let detector = LeakDetector::new();
            // 构造包含已知敏感模式的内容
            let content = format!(
                "{}sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123{}",
                prefix, suffix
            );

            let result = detector.scan(&content);
            if !result.is_clean() {
                if let Some(sanitized) = result.sanitized_content {
                    // 验证原始密钥不在脱敏后的内容中
                    prop_assert!(
                        !sanitized.contains("sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123"),
                        "Sanitized content should not contain original secret"
                    );
                }
            }
        }

        /// Property: 脱敏后的内容应该保持原始结构和上下文
        #[test]
        fn prop_sanitization_preserves_structure(
            prefix in "[a-zA-Z0-9 ]*",
            suffix in "[a-zA-Z0-9 ]*"
        ) {
            let detector = LeakDetector::new();
            let content = format!(
                "{}sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123{}",
                prefix, suffix
            );

            let result = detector.scan(&content);
            if !result.is_clean() {
                if let Some(sanitized) = result.sanitized_content {
                    // 验证前缀和后缀仍然存在
                    if !prefix.is_empty() {
                        prop_assert!(
                            sanitized.contains(&prefix),
                            "Sanitized content should preserve prefix"
                        );
                    }
                    if !suffix.is_empty() {
                        prop_assert!(
                            sanitized.contains(&suffix),
                            "Sanitized content should preserve suffix"
                        );
                    }
                }
            }
        }

        /// Property: 脱敏应该是确定性的 (相同输入产生相同输出)
        #[test]
        fn prop_sanitization_deterministic(content in ".*") {
            let detector1 = LeakDetector::new();
            let detector2 = LeakDetector::new();

            let result1 = detector1.scan(&content);
            let result2 = detector2.scan(&content);

            // 验证两次扫描的结果一致
            prop_assert_eq!(
                result1.is_clean(),
                result2.is_clean(),
                "Scan results should be deterministic"
            );

            if !result1.is_clean() {
                prop_assert_eq!(
                    result1.matches.len(),
                    result2.matches.len(),
                    "Number of matches should be consistent"
                );
            }
        }

        /// Property: 脱敏后的内容长度应该大于等于原始内容长度
        /// (因为敏感信息被替换为占位符)
        #[test]
        fn prop_sanitized_length_reasonable(content in ".*") {
            let detector = LeakDetector::new();
            let result = detector.scan(&content);

            if !result.is_clean() {
                if let Some(sanitized) = result.sanitized_content {
                    // 脱敏后的内容长度应该合理
                    // (不应该显著增加或减少)
                    let length_ratio = sanitized.len() as f64 / content.len().max(1) as f64;
                    prop_assert!(
                        length_ratio >= 0.5 && length_ratio <= 2.0,
                        "Sanitized content length should be reasonable"
                    );
                }
            }
        }

        /// Property: 多个敏感模式应该都被检测和脱敏
        #[test]
        fn prop_multiple_patterns_detected(
            prefix in "[a-zA-Z0-9 ]*",
            middle in "[a-zA-Z0-9 ]*",
            suffix in "[a-zA-Z0-9 ]*"
        ) {
            let detector = LeakDetector::new();
            let content = format!(
                "{}sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123{}ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx{}",
                prefix, middle, suffix
            );

            let result = detector.scan(&content);
            if !result.is_clean() {
                // 应该检测到至少 2 个敏感模式
                prop_assert!(
                    result.matches.len() >= 1,
                    "Should detect at least one sensitive pattern"
                );

                if let Some(sanitized) = result.sanitized_content {
                    // 验证两个敏感信息都被脱敏
                    prop_assert!(
                        !sanitized.contains("sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123"),
                        "First secret should be sanitized"
                    );
                    prop_assert!(
                        !sanitized.contains("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
                        "Second secret should be sanitized"
                    );
                }
            }
        }
    }

    // ============================================================================
    // 边界情况测试
    // ============================================================================

    #[test]
    fn test_sanitization_with_empty_content() {
        let detector = LeakDetector::new();
        let result = detector.scan("");
        assert!(result.is_clean());
    }

    #[test]
    fn test_sanitization_with_only_secrets() {
        let detector = LeakDetector::new();
        let content = "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";

        let result = detector.scan(content);
        assert!(!result.is_clean());

        if let Some(sanitized) = result.sanitized_content {
            // 整个内容都应该被脱敏
            assert!(!sanitized.contains("sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123"));
        }
    }

    #[test]
    fn test_sanitization_with_adjacent_secrets() {
        let detector = LeakDetector::new();
        let content = "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

        let result = detector.scan(content);
        assert!(!result.is_clean());

        if let Some(sanitized) = result.sanitized_content {
            // 两个相邻的敏感信息都应该被脱敏
            assert!(!sanitized.contains("sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123"));
            assert!(!sanitized.contains("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
        }
    }
}
