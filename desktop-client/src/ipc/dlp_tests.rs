//! DLP 桥接命令测试。
//!
//! 测试维度：
//! - 单元测试：类型转换、响应格式
//! - 契约测试：前端期望的字段名和类型
//! - 安全审计测试：敏感信息不泄露
//! - 失败路径测试：异常输入处理

#[cfg(test)]
mod tests {
    use crate::ipc::dlp::*;
    use crate::safety_bridge::{BridgeScanResult, BridgeStats};
    use ironclaw::safety::{SafetyConfig, SafetyLayer};
    use std::sync::Arc;

    // ========================================================================
    // 单元测试：类型转换
    // ========================================================================

    #[test]
    fn test_clean_content_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: false,
            sanitized_content: "hello world".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats::default(),
        };

        let response = DlpScanResponse::from(bridge_result);

        assert!(!response.had_sensitive_data);
        assert_eq!(response.sanitized_content, "hello world");
        assert!(!response.was_blocked);
        assert!(response.block_reason.is_none());
        assert_eq!(response.sanitization_stats.total_matches, 0);
        assert_eq!(response.sanitization_stats.redacted_count, 0);
        assert_eq!(response.sanitization_stats.blocked_count, 0);
        assert_eq!(response.sanitization_stats.warned_count, 0);
    }

    #[test]
    fn test_pii_detected_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: "身份证 110***234".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats {
                secret_detected: false,
                pii_matches: 1,
                redacted_count: 1,
                blocked_count: 0,
                warned_count: 0,
            },
        };

        let response = DlpScanResponse::from(bridge_result);

        assert!(response.had_sensitive_data);
        assert_eq!(response.sanitized_content, "身份证 110***234");
        assert!(!response.was_blocked);
        assert_eq!(response.sanitization_stats.total_matches, 1);
        assert_eq!(response.sanitization_stats.redacted_count, 1);
    }

    #[test]
    fn test_secret_blocked_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: String::new(),
            was_blocked: true,
            block_reason: Some("Secret detected".to_string()),
            stats: BridgeStats {
                secret_detected: true,
                pii_matches: 0,
                redacted_count: 0,
                blocked_count: 0,
                warned_count: 0,
            },
        };

        let response = DlpScanResponse::from(bridge_result);

        assert!(response.had_sensitive_data);
        assert!(response.sanitized_content.is_empty());
        assert!(response.was_blocked);
        assert_eq!(response.block_reason.as_deref(), Some("Secret detected"));
        // secret_detected 计入 total_matches 和 blocked_count
        assert_eq!(response.sanitization_stats.total_matches, 1);
        assert_eq!(response.sanitization_stats.blocked_count, 1);
    }

    #[test]
    fn test_multiple_pii_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: "身份证 110***234 手机 138***000".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats {
                secret_detected: false,
                pii_matches: 2,
                redacted_count: 2,
                blocked_count: 0,
                warned_count: 1,
            },
        };

        let response = DlpScanResponse::from(bridge_result);

        assert_eq!(response.sanitization_stats.total_matches, 2);
        assert_eq!(response.sanitization_stats.redacted_count, 2);
        assert_eq!(response.sanitization_stats.warned_count, 1);
    }

    // ========================================================================
    // 契约测试：前端期望的 JSON 字段名
    // ========================================================================

    #[test]
    fn test_contract_scan_response_json_fields() {
        let response = DlpScanResponse {
            had_sensitive_data: true,
            sanitized_content: "test".to_string(),
            was_blocked: false,
            block_reason: None,
            sanitization_stats: DlpScanStats {
                total_matches: 1,
                redacted_count: 1,
                blocked_count: 0,
                warned_count: 0,
            },
        };

        let json = serde_json::to_value(&response).unwrap();

        // 验证前端 useDlpScan.ts 期望的字段名
        assert!(json.get("had_sensitive_data").is_some());
        assert!(json.get("sanitized_content").is_some());
        assert!(json.get("was_blocked").is_some());
        assert!(json.get("block_reason").is_some());
        assert!(json.get("sanitization_stats").is_some());

        let stats = json.get("sanitization_stats").unwrap();
        assert!(stats.get("total_matches").is_some());
        assert!(stats.get("redacted_count").is_some());
        assert!(stats.get("blocked_count").is_some());
        assert!(stats.get("warned_count").is_some());
    }

    #[test]
    fn test_contract_statistics_response_json_fields() {
        let response = DlpStatisticsResponse {
            total_scans: 100,
            sensitive_data_detected: 10,
            content_blocked: 2,
            content_sanitized: 8,
            http_requests_blocked: 0,
        };

        let json = serde_json::to_value(&response).unwrap();

        // 验证前端 useDlpScan.ts 期望的字段名
        assert!(json.get("total_scans").is_some());
        assert!(json.get("sensitive_data_detected").is_some());
        assert!(json.get("content_blocked").is_some());
        assert!(json.get("content_sanitized").is_some());
        assert!(json.get("http_requests_blocked").is_some());
    }

    #[test]
    fn test_contract_config_response_json_fields() {
        let response = DlpConfigResponse {
            enabled: true,
            sanitization: DlpSanitizationConfig {
                redaction_text: "[敏感信息]".to_string(),
                preserve_format: true,
                partial_redaction: true,
            },
            real_time_monitoring: true,
            audit_logging: true,
            custom_patterns: vec![],
        };

        let json = serde_json::to_value(&response).unwrap();

        // 验证前端 useDlpScan.ts 期望的字段名
        assert!(json.get("enabled").is_some());
        assert!(json.get("sanitization").is_some());
        assert!(json.get("real_time_monitoring").is_some());
        assert!(json.get("audit_logging").is_some());
        assert!(json.get("custom_patterns").is_some());

        let sanitization = json.get("sanitization").unwrap();
        assert!(sanitization.get("redaction_text").is_some());
        assert!(sanitization.get("preserve_format").is_some());
        assert!(sanitization.get("partial_redaction").is_some());
    }

    #[test]
    fn test_contract_sync_result_json_fields() {
        let result = SyncDlpResult {
            success: true,
            rules_synced: 0,
            message: "test".to_string(),
        };

        let json = serde_json::to_value(&result).unwrap();

        assert!(json.get("success").is_some());
        assert!(json.get("rules_synced").is_some());
        assert!(json.get("message").is_some());
    }

    #[test]
    fn test_contract_response_roundtrip() {
        // 验证序列化 → 反序列化一致性
        let original = DlpScanResponse {
            had_sensitive_data: true,
            sanitized_content: "身份证 110***234".to_string(),
            was_blocked: false,
            block_reason: None,
            sanitization_stats: DlpScanStats {
                total_matches: 1,
                redacted_count: 1,
                blocked_count: 0,
                warned_count: 0,
            },
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: DlpScanResponse = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original.had_sensitive_data, deserialized.had_sensitive_data);
        assert_eq!(original.sanitized_content, deserialized.sanitized_content);
        assert_eq!(original.was_blocked, deserialized.was_blocked);
        assert_eq!(
            original.sanitization_stats.total_matches,
            deserialized.sanitization_stats.total_matches
        );
    }

    // ========================================================================
    // 安全审计测试
    // ========================================================================

    #[test]
    fn test_audit_blocked_response_no_original_content() {
        // 被阻止时，sanitized_content 应为空，不包含原始敏感信息
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: String::new(),
            was_blocked: true,
            block_reason: Some("Secret detected".to_string()),
            stats: BridgeStats {
                secret_detected: true,
                ..Default::default()
            },
        };

        let response = DlpScanResponse::from(bridge_result);
        let json_str = serde_json::to_string(&response).unwrap();

        // 验证 JSON 中不包含任何可能的密钥模式
        assert!(!json_str.contains("ghp_"));
        assert!(!json_str.contains("sk-"));
        assert!(!json_str.contains("AKIA"));
        assert!(response.sanitized_content.is_empty());
    }

    #[test]
    fn test_audit_block_reason_no_sensitive_data() {
        // block_reason 不应包含原始敏感数据
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: String::new(),
            was_blocked: true,
            block_reason: Some("Potential secret or API key detected".to_string()),
            stats: BridgeStats {
                secret_detected: true,
                ..Default::default()
            },
        };

        let response = DlpScanResponse::from(bridge_result);

        // block_reason 应该是通用描述，不包含实际密钥
        if let Some(reason) = &response.block_reason {
            assert!(!reason.contains("ghp_"));
            assert!(!reason.contains("sk-"));
            assert!(!reason.contains("110101199003071234"));
        }
    }

    #[test]
    fn test_audit_statistics_no_content_leak() {
        // 统计信息不应包含任何原始内容
        let response = DlpStatisticsResponse {
            total_scans: 100,
            sensitive_data_detected: 10,
            content_blocked: 2,
            content_sanitized: 8,
            http_requests_blocked: 0,
        };

        let json_str = serde_json::to_string(&response).unwrap();

        // 统计信息只有数字，不应包含任何文本内容
        assert!(!json_str.contains("身份证"));
        assert!(!json_str.contains("手机"));
        assert!(!json_str.contains("ghp_"));
    }

    // ========================================================================
    // 失败路径测试
    // ========================================================================

    #[test]
    fn test_failure_empty_content_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: false,
            sanitized_content: String::new(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats::default(),
        };

        let response = DlpScanResponse::from(bridge_result);

        assert!(!response.had_sensitive_data);
        assert!(response.sanitized_content.is_empty());
        assert!(!response.was_blocked);
    }

    #[test]
    fn test_failure_default_stats_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: false,
            sanitized_content: "test".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats::default(),
        };

        let response = DlpScanResponse::from(bridge_result);

        assert_eq!(response.sanitization_stats.total_matches, 0);
        assert_eq!(response.sanitization_stats.redacted_count, 0);
        assert_eq!(response.sanitization_stats.blocked_count, 0);
        assert_eq!(response.sanitization_stats.warned_count, 0);
    }

    #[test]
    fn test_failure_unicode_content_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: false,
            sanitized_content: "🔑 密码 العربية emoji 🎉".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats::default(),
        };

        let response = DlpScanResponse::from(bridge_result);
        assert_eq!(response.sanitized_content, "🔑 密码 العربية emoji 🎉");
    }

    #[tokio::test]
    async fn req_dlp_i3_admin_rule_sync_updates_scan_behavior() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/api/dlp-rules")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "rules": [
                        {
                            "name": "critical-token",
                            "pattern": "ACME-TOKEN-[0-9]+",
                            "rule_type": "regex",
                            "severity": "critical",
                            "enabled": true
                        }
                    ]
                }"#,
            )
            .create_async()
            .await;

        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));
        let bridge = crate::safety_bridge::SafetyBridge::new(safety, None, None);

        let sync = do_sync_dlp_rules_from_admin_url(&bridge, &server.url())
            .await
            .expect("rules sync");
        assert!(sync.success);
        assert_eq!(sync.rules_synced, 1);

        let scan = bridge.scan_user_input("please handle ACME-TOKEN-42");
        assert!(scan.had_sensitive_data);
        assert!(scan.was_blocked);
        assert!(scan.sanitized_content.is_empty());
    }

    #[test]
    fn test_failure_large_stats_conversion() {
        let bridge_result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: "redacted".to_string(),
            was_blocked: false,
            block_reason: None,
            stats: BridgeStats {
                secret_detected: false,
                pii_matches: usize::MAX,
                redacted_count: usize::MAX,
                blocked_count: 0,
                warned_count: 0,
            },
        };

        let response = DlpScanResponse::from(bridge_result);

        // 不应 panic，大数值正常处理
        assert_eq!(response.sanitization_stats.total_matches, usize::MAX);
        assert_eq!(response.sanitization_stats.redacted_count, usize::MAX);
    }
}
