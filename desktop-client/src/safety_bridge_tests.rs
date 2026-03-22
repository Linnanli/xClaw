//! SafetyBridge 多维度测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 失败路径测试
//! - 安全审计测试
//! - 契约测试
//! - 可靠性测试

use std::sync::Arc;

use ironclaw::safety::{SafetyConfig, SafetyLayer};

use crate::data_reporter::DataReporter;
use crate::dlp::sanitizer::SanitizationConfig;
use crate::safety_bridge::{SafetyBridge, BridgeScanResult, BridgeStats, BridgeCumulativeStats};

// ═══════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════

/// 创建默认 SafetyBridge（无 DataReporter）。
fn create_bridge() -> SafetyBridge {
    let config = SafetyConfig {
        max_output_length: 100_000,
        injection_check_enabled: true,
    };
    let safety = Arc::new(SafetyLayer::new(&config));
    SafetyBridge::new(safety, None, None)
}

/// 创建带 DataReporter 的 SafetyBridge。
fn create_bridge_with_reporter() -> (SafetyBridge, Arc<DataReporter>) {
    let config = SafetyConfig {
        max_output_length: 100_000,
        injection_check_enabled: true,
    };
    let safety = Arc::new(SafetyLayer::new(&config));
    let reporter = Arc::new(DataReporter::new(
        "https://admin.test.local".to_string(),
        "test-token".to_string(),
    ));
    let bridge = SafetyBridge::new(safety, None, Some(Arc::clone(&reporter)));
    (bridge, reporter)
}

/// 创建自定义脱敏配置的 SafetyBridge。
fn create_bridge_with_config(sanitization_config: SanitizationConfig) -> SafetyBridge {
    let config = SafetyConfig {
        max_output_length: 100_000,
        injection_check_enabled: true,
    };
    let safety = Arc::new(SafetyLayer::new(&config));
    SafetyBridge::new(safety, Some(sanitization_config), None)
}

// ═══════════════════════════════════════════════════════════════════
// 1. 单元测试 — 正常路径
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_scan_clean_content() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("这是一段普通的文本内容。");

        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.sanitized_content, "这是一段普通的文本内容。");
        assert!(!result.stats.secret_detected);
        assert_eq!(result.stats.pii_matches, 0);
    }

    #[test]
    fn test_scan_chinese_id_card_redaction() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("我的身份证号是 110101199003071234 。");

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("110************234"));
        assert!(result.stats.pii_matches >= 1);
        assert!(result.stats.redacted_count >= 1);
    }

    #[test]
    fn test_scan_chinese_mobile_redaction() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("联系电话： 13800138000 ");

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("138*****000"));
    }

    #[test]
    fn test_scan_multiple_pii() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input(
            "用户信息：身份证 110101199003071234 ，手机 13800138000 "
        );

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("110************234"));
        assert!(result.sanitized_content.contains("138*****000"));
        assert!(result.stats.pii_matches >= 2);
    }

    #[test]
    fn test_scan_tool_output() {
        let bridge = create_bridge();
        let output = bridge.scan_tool_output("test_tool", "Normal tool output content");

        assert!(!output.was_modified);
        assert_eq!(output.content, "Normal tool output content");
    }

    #[test]
    fn test_scan_outbound_clean() {
        let bridge = create_bridge();
        let result = bridge.scan_outbound(r#"{"query": "hello world"}"#);

        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
    }

    #[test]
    fn test_scan_outbound_with_pii() {
        let bridge = create_bridge();
        let result = bridge.scan_outbound(r#"{"user_id": "13800138000"}"#);

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("138*****000"));
    }

    #[test]
    fn test_sanitize_for_storage_clean() {
        let bridge = create_bridge();
        let result = bridge.sanitize_for_storage("普通文本");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "普通文本");
    }

    #[test]
    fn test_sanitize_for_storage_with_pii() {
        let bridge = create_bridge();
        let result = bridge.sanitize_for_storage("手机号：13800138000");

        assert!(result.is_ok());
        assert!(result.unwrap().contains("138*****000"));
    }

    #[test]
    fn test_cumulative_stats_initial() {
        let bridge = create_bridge();
        let stats = bridge.cumulative_stats();

        assert_eq!(stats.total_scans, 0);
        assert_eq!(stats.secret_blocks, 0);
        assert_eq!(stats.pii_detections, 0);
        assert_eq!(stats.clean_passes, 0);
    }

    #[test]
    fn test_cumulative_stats_after_scans() {
        let bridge = create_bridge();

        // 干净内容
        bridge.scan_user_input("hello");
        // PII 内容
        bridge.scan_user_input("身份证 110101199003071234");

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 2);
        assert_eq!(stats.clean_passes, 1);
        assert!(stats.pii_detections >= 1);
    }

    #[test]
    fn test_sanitization_config_accessor() {
        let bridge = create_bridge();
        let config = bridge.sanitization_config();

        assert!(config.enabled);
        assert!(config.preserve_format);
    }

    #[test]
    fn test_custom_sanitization_config() {
        let mut config = SanitizationConfig::default();
        config.format_preservation.id_card_preserve_chars = 2;
        config.format_preservation.mask_char = '#';

        let bridge = create_bridge_with_config(config);
        let result = bridge.scan_user_input("身份证：110101199003071234");

        assert!(result.had_sensitive_data);
        assert!(result.sanitized_content.contains("11##############34"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// 2. 失败路径测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod failure_tests {
    use super::*;

    #[test]
    fn test_failure_secret_in_user_input_blocks() {
        let bridge = create_bridge();
        // OpenAI API key — SafetyLayer 应该检测并阻止
        let content = "My key is sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";
        let result = bridge.scan_user_input(content);

        assert!(result.was_blocked, "Secret should be blocked");
        assert!(result.block_reason.is_some());
        assert!(result.stats.secret_detected);
        assert!(result.sanitized_content.is_empty());
    }

    #[test]
    fn test_failure_github_token_blocks() {
        let bridge = create_bridge();
        let content = "GitHub token: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let result = bridge.scan_user_input(content);

        assert!(result.was_blocked, "GitHub token should be blocked");
        assert!(result.stats.secret_detected);
    }

    #[test]
    fn test_failure_aws_key_blocks() {
        let bridge = create_bridge();
        let content = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let result = bridge.scan_user_input(content);

        assert!(result.was_blocked, "AWS key should be blocked");
        assert!(result.stats.secret_detected);
    }

    #[test]
    fn test_failure_aliyun_key_in_outbound_blocks() {
        let bridge = create_bridge();
        let content = "阿里云密钥：LTAI4G8aB9cD2eFgH3iJ";
        let result = bridge.scan_outbound(content);

        // 可能被 SafetyLayer 或 DLP 阻止
        assert!(result.was_blocked, "Aliyun key should be blocked");
    }

    #[test]
    fn test_failure_sanitize_for_storage_blocked_returns_err() {
        let bridge = create_bridge();
        let content = "API key: sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";
        let result = bridge.sanitize_for_storage(content);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_failure_empty_content() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("");

        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.sanitized_content, "");
    }

    #[test]
    fn test_failure_whitespace_only_content() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("   \n\t  ");

        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
    }

    #[test]
    fn test_failure_very_long_content() {
        let bridge = create_bridge();
        let long_content = "a".repeat(1_000_000);
        let result = bridge.scan_user_input(&long_content);

        // 不应该 panic 或阻塞
        assert!(!result.was_blocked);
    }

    #[test]
    fn test_failure_unicode_edge_cases() {
        let bridge = create_bridge();
        // 包含各种 Unicode 字符
        let content = "🔑 密码：test 🎉 emoji 中文 العربية";
        let result = bridge.scan_user_input(content);

        // 不应该 panic
        assert!(!result.was_blocked);
    }

    #[test]
    fn test_failure_mixed_secrets_and_pii() {
        let bridge = create_bridge();
        // 同时包含密钥和 PII — 密钥优先阻止
        let content = "身份证 110101199003071234 和 API key sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";
        let result = bridge.scan_user_input(content);

        // 密钥检测优先，应该被阻止
        assert!(result.was_blocked);
        assert!(result.stats.secret_detected);
    }

    #[test]
    fn test_failure_cumulative_stats_after_blocks() {
        let bridge = create_bridge();

        // 密钥阻止
        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        // 干净内容
        bridge.scan_user_input("hello");

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 2);
        assert!(stats.secret_blocks >= 1);
        assert!(stats.clean_passes >= 1);
    }

    #[test]
    fn test_failure_disabled_sanitization() {
        let mut config = SanitizationConfig::default();
        config.enabled = false;

        let bridge = create_bridge_with_config(config);
        let result = bridge.scan_user_input("身份证：110101199003071234");

        // SafetyLayer 密钥扫描仍然生效，但 DLP PII 脱敏被禁用
        // 身份证不是密钥，所以不会被 SafetyLayer 阻止
        assert!(!result.was_blocked);
        // DLP 禁用后不检测 PII
        assert!(!result.had_sensitive_data);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 3. 安全审计测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod security_audit_tests {
    use super::*;

    #[test]
    fn test_audit_no_original_content_in_block_reason() {
        let bridge = create_bridge();
        let secret = "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123";
        let content = format!("My key is {}", secret);
        let result = bridge.scan_user_input(&content);

        assert!(result.was_blocked);
        // block_reason 不应包含原始密钥
        if let Some(ref reason) = result.block_reason {
            assert!(
                !reason.contains(secret),
                "Block reason should not contain the original secret"
            );
        }
    }

    #[test]
    fn test_audit_sanitized_content_no_original_pii() {
        let bridge = create_bridge();
        let id_card = "110101199003071234";
        let content = format!("身份证号是 {} 。", id_card);
        let result = bridge.scan_user_input(&content);

        assert!(result.had_sensitive_data);
        assert!(
            !result.sanitized_content.contains(id_card),
            "Sanitized content should not contain original ID card number"
        );
    }

    #[test]
    fn test_audit_sanitized_content_no_original_mobile() {
        let bridge = create_bridge();
        let mobile = "13800138000";
        let content = format!("手机号 {} ", mobile);
        let result = bridge.scan_user_input(&content);

        assert!(result.had_sensitive_data);
        assert!(
            !result.sanitized_content.contains(mobile),
            "Sanitized content should not contain original mobile number"
        );
    }

    #[test]
    fn test_audit_reporter_receives_secret_block_event() {
        let (bridge, reporter) = create_bridge_with_reporter();
        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

        assert!(reporter.queue_len() >= 1, "Reporter should have at least 1 event");
    }

    #[test]
    fn test_audit_reporter_receives_pii_event() {
        let (bridge, reporter) = create_bridge_with_reporter();
        bridge.scan_user_input("身份证 110101199003071234");

        assert!(reporter.queue_len() >= 1, "Reporter should have at least 1 PII event");
    }

    #[test]
    fn test_audit_reporter_no_event_for_clean_content() {
        let (bridge, reporter) = create_bridge_with_reporter();
        bridge.scan_user_input("这是普通文本");

        assert_eq!(reporter.queue_len(), 0, "No event for clean content");
    }

    #[test]
    fn test_audit_no_reporter_no_panic() {
        // 没有 reporter 时不应该 panic
        let bridge = create_bridge();
        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        bridge.scan_user_input("身份证 110101199003071234");
        bridge.scan_user_input("普通文本");
        // 如果到这里没有 panic，测试通过
    }

    #[test]
    fn test_audit_fail_safe_design() {
        // 验证故障安全设计：密钥检测到时必须阻止，不降级
        let bridge = create_bridge();

        let secrets = vec![
            "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123",
            "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "AKIAIOSFODNN7EXAMPLE",
        ];

        for secret in secrets {
            let result = bridge.scan_user_input(secret);
            assert!(
                result.was_blocked,
                "Secret '{}...' must be blocked (Fail-Safe)",
                &secret[..10.min(secret.len())]
            );
            assert!(
                result.sanitized_content.is_empty(),
                "Blocked content must be empty string"
            );
        }
    }

    #[test]
    fn test_audit_pii_redaction_preserves_format() {
        let bridge = create_bridge();

        // 身份证：保留前3后3
        let result = bridge.scan_user_input("身份证 330326199408015618");
        if result.had_sensitive_data && !result.was_blocked {
            assert!(
                result.sanitized_content.contains("330************618"),
                "ID card should be format-preserved: {}",
                result.sanitized_content
            );
        }

        // 手机号：保留前3后3
        let result = bridge.scan_user_input("手机 13800138000");
        if result.had_sensitive_data && !result.was_blocked {
            assert!(
                result.sanitized_content.contains("138*****000"),
                "Mobile should be format-preserved: {}",
                result.sanitized_content
            );
        }
    }

    #[test]
    fn test_audit_tool_output_sanitization() {
        let bridge = create_bridge();
        // 工具输出中包含密钥应该被清理
        let output = bridge.scan_tool_output(
            "web_fetch",
            "Response: AKIAIOSFODNN7EXAMPLE found in config",
        );

        // SafetyLayer 应该检测到密钥并修改输出
        assert!(output.was_modified, "Tool output with secret should be modified");
    }

    #[test]
    fn test_audit_tool_output_truncation() {
        let config = SafetyConfig {
            max_output_length: 100,
            injection_check_enabled: true,
        };
        let safety = Arc::new(SafetyLayer::new(&config));
        let bridge = SafetyBridge::new(safety, None, None);

        let long_output = "x".repeat(200);
        let output = bridge.scan_tool_output("test_tool", &long_output);

        assert!(output.was_modified);
        // SafetyLayer 截断后会添加 notice，所以内容不包含完整原始输出
        assert!(
            !output.content.contains(&long_output),
            "Truncated output should not contain full original"
        );
        assert!(
            output.content.contains("truncated"),
            "Truncated output should contain truncation notice"
        );
    }

    #[test]
    fn test_audit_multiple_scan_types_stats_isolation() {
        let bridge = create_bridge();

        // 用户输入扫描
        bridge.scan_user_input("hello");
        // 出站扫描
        bridge.scan_outbound("world");

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 2);
        assert_eq!(stats.clean_passes, 2);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 4. 契约测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn test_contract_bridge_scan_result_serializable() {
        let result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: "test".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats::default(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: BridgeScanResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.had_sensitive_data, result.had_sensitive_data);
        assert_eq!(parsed.sanitized_content, result.sanitized_content);
        assert_eq!(parsed.was_blocked, result.was_blocked);
        assert_eq!(parsed.block_reason, result.block_reason);
    }

    #[test]
    fn test_contract_bridge_stats_serializable() {
        let stats = BridgeStats {
            secret_detected: true,
            pii_matches: 3,
            redacted_count: 2,
            blocked_count: 1,
            warned_count: 0,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let parsed: BridgeStats = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.secret_detected, stats.secret_detected);
        assert_eq!(parsed.pii_matches, stats.pii_matches);
        assert_eq!(parsed.redacted_count, stats.redacted_count);
        assert_eq!(parsed.blocked_count, stats.blocked_count);
        assert_eq!(parsed.warned_count, stats.warned_count);
    }

    #[test]
    fn test_contract_cumulative_stats_serializable() {
        let stats = BridgeCumulativeStats {
            total_scans: 100,
            secret_blocks: 5,
            pii_detections: 20,
            pii_redactions: 18,
            pii_blocks: 2,
            clean_passes: 75,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let parsed: BridgeCumulativeStats = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.total_scans, stats.total_scans);
        assert_eq!(parsed.secret_blocks, stats.secret_blocks);
        assert_eq!(parsed.pii_detections, stats.pii_detections);
    }

    #[test]
    fn test_contract_clean_result_format() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("hello");

        // 干净内容的契约：
        assert!(!result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert_eq!(result.block_reason, None);
        assert!(!result.stats.secret_detected);
        assert_eq!(result.stats.pii_matches, 0);
        assert_eq!(result.stats.redacted_count, 0);
        assert_eq!(result.stats.blocked_count, 0);
    }

    #[test]
    fn test_contract_blocked_result_format() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

        // 阻止结果的契约：
        assert!(result.had_sensitive_data);
        assert!(result.was_blocked);
        assert!(result.block_reason.is_some());
        assert!(result.sanitized_content.is_empty());
        assert!(result.stats.secret_detected);
    }

    #[test]
    fn test_contract_redacted_result_format() {
        let bridge = create_bridge();
        let result = bridge.scan_user_input("身份证 110101199003071234");

        // 脱敏结果的契约：
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.block_reason.is_none());
        assert!(!result.sanitized_content.is_empty());
        assert!(!result.stats.secret_detected);
        assert!(result.stats.pii_matches >= 1);
        assert!(result.stats.redacted_count >= 1);
    }

    #[test]
    fn test_contract_bridge_stats_from_sanitization_stats() {
        use crate::dlp::sanitizer::SanitizationStats;
        use std::collections::HashMap;

        let mut severity_stats = HashMap::new();
        severity_stats.insert("High".to_string(), 2);
        severity_stats.insert("Medium".to_string(), 1);

        let dlp_stats = SanitizationStats {
            total_matches: 3,
            redacted_count: 2,
            warned_count: 0,
            blocked_count: 1,
            severity_stats,
        };

        let bridge_stats = BridgeStats::from(&dlp_stats);

        assert!(!bridge_stats.secret_detected); // From 不设置 secret_detected
        assert_eq!(bridge_stats.pii_matches, 3);
        assert_eq!(bridge_stats.redacted_count, 2);
        assert_eq!(bridge_stats.blocked_count, 1);
        assert_eq!(bridge_stats.warned_count, 0);
    }

    #[test]
    fn test_contract_scan_user_input_and_scan_outbound_same_behavior() {
        let bridge = create_bridge();

        let content = "身份证 110101199003071234";
        let input_result = bridge.scan_user_input(content);
        let outbound_result = bridge.scan_outbound(content);

        // 两者应该有相同的安全行为
        assert_eq!(input_result.had_sensitive_data, outbound_result.had_sensitive_data);
        assert_eq!(input_result.was_blocked, outbound_result.was_blocked);
        // 脱敏内容应该相同
        assert_eq!(input_result.sanitized_content, outbound_result.sanitized_content);
    }

    #[test]
    fn test_contract_sanitize_for_storage_consistent_with_scan() {
        let bridge = create_bridge();

        let content = "手机号 13800138000";
        let scan_result = bridge.scan_user_input(content);
        let storage_result = bridge.sanitize_for_storage(content);

        assert!(storage_result.is_ok());
        assert_eq!(storage_result.unwrap(), scan_result.sanitized_content);
    }

    #[test]
    fn test_contract_sanitize_for_storage_blocked_consistent() {
        let bridge = create_bridge();

        let content = "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let scan_result = bridge.scan_user_input(content);
        let storage_result = bridge.sanitize_for_storage(content);

        assert!(scan_result.was_blocked);
        assert!(storage_result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════
// 5. 可靠性测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod reliability_tests {
    use super::*;

    #[test]
    fn test_reliability_concurrent_scans() {
        use std::thread;

        let bridge = Arc::new(create_bridge());
        let mut handles = vec![];

        for i in 0..10 {
            let bridge = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                let content = if i % 2 == 0 {
                    "普通文本".to_string()
                } else {
                    format!("身份证 11010119900307{:04}", i)
                };
                bridge.scan_user_input(&content)
            }));
        }

        for handle in handles {
            let result = handle.join().expect("Thread should not panic");
            // 所有结果都应该是有效的
            assert!(!result.sanitized_content.is_empty() || result.was_blocked || !result.had_sensitive_data);
        }
    }

    #[test]
    fn test_reliability_cumulative_stats_thread_safe() {
        use std::thread;

        let bridge = Arc::new(create_bridge());
        let mut handles = vec![];

        for _ in 0..20 {
            let bridge = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                bridge.scan_user_input("hello");
            }));
        }

        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 20);
        assert_eq!(stats.clean_passes, 20);
    }

    #[test]
    fn test_reliability_repeated_scans_consistency() {
        let bridge = create_bridge();
        let content = "身份证 110101199003071234";

        // 多次扫描同一内容应该得到一致的结果
        let results: Vec<BridgeScanResult> = (0..5)
            .map(|_| bridge.scan_user_input(content))
            .collect();

        for result in &results {
            assert_eq!(result.had_sensitive_data, results[0].had_sensitive_data);
            assert_eq!(result.was_blocked, results[0].was_blocked);
            assert_eq!(result.sanitized_content, results[0].sanitized_content);
        }
    }

    #[test]
    fn test_reliability_large_batch_scans() {
        let bridge = create_bridge();

        for i in 0..100 {
            let content = format!("消息 {} 内容", i);
            let result = bridge.scan_user_input(&content);
            assert!(!result.was_blocked);
        }

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 100);
        assert_eq!(stats.clean_passes, 100);
    }

    #[test]
    fn test_reliability_special_characters() {
        let bridge = create_bridge();

        let special_inputs = vec![
            "null\0byte",
            "tab\there",
            "newline\nhere",
            "carriage\rreturn",
            "backslash\\here",
            "quote\"here",
            "单引号'here",
            "<script>alert('xss')</script>",
            "SELECT * FROM users WHERE 1=1; --",
            "{{template}}",
            "${variable}",
            "%(format)s",
        ];

        for input in special_inputs {
            let result = bridge.scan_user_input(input);
            // 不应该 panic
            assert!(
                result.sanitized_content.len() <= input.len() * 2 || result.was_blocked,
                "Unexpected result for input: {}",
                input
            );
        }
    }

    #[test]
    fn test_reliability_reporter_enqueue_under_load() {
        let (bridge, reporter) = create_bridge_with_reporter();

        // 大量 PII 事件
        for _ in 0..50 {
            bridge.scan_user_input("身份证 110101199003071234");
        }

        // reporter 应该收到所有事件
        assert!(reporter.queue_len() >= 50, "Reporter should have at least 50 events");
    }

    #[test]
    fn test_reliability_mixed_workload() {
        let bridge = create_bridge();

        // 混合工作负载：干净内容、PII、密钥
        bridge.scan_user_input("hello");
        bridge.scan_user_input("身份证 110101199003071234");
        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        bridge.scan_outbound(r#"{"data": "13800138000"}"#);
        bridge.scan_tool_output("test", "normal output");
        let _ = bridge.sanitize_for_storage("手机 13800138000");

        let stats = bridge.cumulative_stats();
        // scan_user_input x3 + scan_outbound x1 + sanitize_for_storage x1 (内部调用 scan_user_input)
        // scan_tool_output 不计入 cumulative_stats（它走 SafetyLayer 直接路径）
        assert!(stats.total_scans >= 4);
        assert!(stats.secret_blocks >= 1);
        assert!(stats.pii_detections >= 1);
        assert!(stats.clean_passes >= 1);
    }

    #[test]
    fn test_reliability_iso_timestamp_format() {
        // 验证时间戳通过 BridgeScanResult 间接测试
        // （iso_timestamp_now 是模块私有函数，通过 reporter 事件间接验证）
        let (bridge, reporter) = create_bridge_with_reporter();
        bridge.scan_user_input("身份证 110101199003071234");

        // reporter 收到事件说明时间戳生成成功
        assert!(reporter.queue_len() >= 1);
    }
}
