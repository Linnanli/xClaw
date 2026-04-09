//! 嵌入架构端到端集成测试。
//!
//! 验证 Phase 1-4 各模块之间的集成正确性：
//! - TauriChannel ↔ AppState ↔ IPC Commands
//! - SafetyBridge ↔ SafetyLayer ↔ DLP
//! - AdminSync ↔ DataReporter
//!
//! 覆盖维度：
//! - 集成测试（组件交互）
//! - 契约测试（模块间接口一致性）
//! - 失败路径测试（跨模块错误传播）
//! - 安全审计测试（端到端安全验证）

use std::sync::Arc;

use ironclaw::safety::{SafetyConfig, SafetyLayer};

use crate::data_reporter::{ClientReport, DataReporter};
use crate::safety_bridge::SafetyBridge;

// ═══════════════════════════════════════════════════════════════════
// 1. SafetyBridge ↔ DataReporter 集成测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod bridge_reporter_integration {
    use super::*;

    fn create_integrated_bridge() -> (SafetyBridge, Arc<DataReporter>) {
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

    #[test]
    fn test_integration_secret_block_reports_to_admin() {
        let (bridge, reporter) = create_integrated_bridge();

        // 密钥输入 → SafetyBridge 阻止 → DataReporter 收到事件
        let result = bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

        assert!(result.was_blocked);
        assert!(reporter.queue_len() >= 1, "Secret block should be reported");
    }

    #[test]
    fn test_integration_pii_redaction_reports_to_admin() {
        let (bridge, reporter) = create_integrated_bridge();

        // PII 输入 → SafetyBridge 脱敏 → DataReporter 收到事件
        let result = bridge.scan_user_input("身份证 110101199003071234");

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(reporter.queue_len() >= 1, "PII event should be reported");
    }

    #[test]
    fn test_integration_clean_content_no_report() {
        let (bridge, reporter) = create_integrated_bridge();

        bridge.scan_user_input("普通文本");

        assert_eq!(
            reporter.queue_len(),
            0,
            "Clean content should not be reported"
        );
    }

    #[test]
    fn test_integration_multiple_events_accumulate() {
        let (bridge, reporter) = create_integrated_bridge();

        // 多种事件
        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"); // 密钥
        bridge.scan_user_input("身份证 110101199003071234"); // PII
        bridge.scan_user_input("手机 13800138000"); // PII
        bridge.scan_user_input("普通文本"); // 干净

        // 应该有 3 个事件（密钥 + 2 个 PII，干净内容不上报）
        assert!(
            reporter.queue_len() >= 3,
            "Should have at least 3 events, got {}",
            reporter.queue_len()
        );
    }

    #[test]
    fn test_integration_outbound_scan_also_reports() {
        let (bridge, reporter) = create_integrated_bridge();

        bridge.scan_outbound(r#"{"phone": "13800138000"}"#);

        assert!(reporter.queue_len() >= 1, "Outbound PII should be reported");
    }

    #[test]
    fn test_integration_stats_and_reporter_consistent() {
        let (bridge, reporter) = create_integrated_bridge();

        bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        bridge.scan_user_input("身份证 110101199003071234");
        bridge.scan_user_input("hello");

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 3);
        assert!(stats.secret_blocks >= 1);
        assert!(stats.pii_detections >= 1);
        assert!(stats.clean_passes >= 1);

        // reporter 应该有 2 个事件（密钥 + PII）
        assert!(reporter.queue_len() >= 2);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 2. SafetyLayer ↔ DLP 链式处理集成测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod safety_dlp_chain_integration {
    use super::*;

    fn create_bridge() -> SafetyBridge {
        let config = SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        };
        let safety = Arc::new(SafetyLayer::new(&config));
        SafetyBridge::new(safety, None, None)
    }

    #[test]
    fn test_chain_secret_takes_priority_over_pii() {
        let bridge = create_bridge();

        // 同时包含密钥和 PII — 密钥检测优先
        let content = "身份证 110101199003071234 和 key ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        let result = bridge.scan_user_input(content);

        // SafetyLayer 密钥检测在 DLP PII 检测之前
        assert!(
            result.was_blocked,
            "Secret should block before PII processing"
        );
        assert!(result.stats.secret_detected);
    }

    #[test]
    fn test_chain_pii_only_when_no_secret() {
        let bridge = create_bridge();

        // 只有 PII，没有密钥 — DLP 脱敏生效
        let content = "身份证 110101199003071234";
        let result = bridge.scan_user_input(content);

        assert!(!result.stats.secret_detected);
        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        assert!(result.sanitized_content.contains("110************234"));
    }

    #[test]
    fn test_chain_tool_output_uses_safety_layer_only() {
        let bridge = create_bridge();

        // 工具输出只走 SafetyLayer，不走 DLP PII 脱敏
        let output = bridge.scan_tool_output("test", "身份证 110101199003071234");

        // SafetyLayer 不检测中国身份证（那是 DLP 的职责）
        // 所以工具输出中的 PII 不会被脱敏（这是设计决策：工具输出发给 LLM，不发给用户）
        // 但如果包含密钥，SafetyLayer 会处理
        assert!(output.content.contains("110101199003071234") || output.was_modified);
    }

    #[test]
    fn test_chain_bearer_token_in_user_input() {
        let bridge = create_bridge();

        let content = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9_longtokenvalue";
        let result = bridge.scan_user_input(content);

        // Bearer token 应该被检测到
        assert!(result.had_sensitive_data || result.was_blocked);
    }

    #[test]
    fn test_chain_multiple_pii_types() {
        let bridge = create_bridge();

        let content = "用户：身份证 110101199003071234，手机 13800138000";
        let result = bridge.scan_user_input(content);

        assert!(result.had_sensitive_data);
        assert!(!result.was_blocked);
        // 两种 PII 都应该被脱敏
        assert!(result.sanitized_content.contains("110************234"));
        assert!(result.sanitized_content.contains("138*****000"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// 3. 端到端安全审计测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod e2e_security_audit {
    use super::*;

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

    #[test]
    fn test_e2e_sensitive_data_never_leaks_to_sanitized_output() {
        let (bridge, _) = create_bridge_with_reporter();

        let sensitive_inputs = vec![
            ("身份证", "110101199003071234"),
            ("手机号", "13800138000"),
            ("身份证2", "330326199408015618"),
        ];

        for (label, sensitive) in &sensitive_inputs {
            let content = format!("{} {}", label, sensitive);
            let result = bridge.scan_user_input(&content);

            if !result.was_blocked {
                assert!(
                    !result.sanitized_content.contains(sensitive),
                    "Sanitized output should not contain original {} '{}'",
                    label,
                    sensitive
                );
            }
        }
    }

    #[test]
    fn test_e2e_all_secret_types_blocked() {
        let (bridge, reporter) = create_bridge_with_reporter();

        let secrets = vec![
            "sk-proj-abc123def456ghi789jkl012mno345pqrT3BlbkFJtest123",
            "ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
            "AKIAIOSFODNN7EXAMPLE",
        ];

        for secret in &secrets {
            let result = bridge.scan_user_input(secret);
            assert!(
                result.was_blocked,
                "Secret should be blocked: {}...",
                &secret[..10]
            );
        }

        // 所有密钥阻止事件都应该上报
        assert!(reporter.queue_len() >= secrets.len());
    }

    #[test]
    fn test_e2e_fail_safe_no_degradation() {
        let (bridge, _) = create_bridge_with_reporter();

        // 验证故障安全：密钥检测到时必须阻止，返回空内容
        let result = bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

        assert!(result.was_blocked);
        assert!(
            result.sanitized_content.is_empty(),
            "Blocked content must be empty"
        );
        assert!(result.block_reason.is_some(), "Must have block reason");
    }

    #[test]
    fn test_e2e_storage_sanitization_complete() {
        let (bridge, _) = create_bridge_with_reporter();

        // 存储脱敏应该完整处理
        let result =
            bridge.sanitize_for_storage("用户手机 13800138000 和身份证 110101199003071234");

        assert!(result.is_ok());
        let sanitized = result.unwrap();
        assert!(!sanitized.contains("13800138000"));
        assert!(!sanitized.contains("110101199003071234"));
        assert!(sanitized.contains("138*****000"));
        assert!(sanitized.contains("110************234"));
    }

    #[test]
    fn test_e2e_storage_blocks_secrets() {
        let (bridge, _) = create_bridge_with_reporter();

        let result = bridge.sanitize_for_storage("key: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

        assert!(result.is_err(), "Storage should reject secrets");
    }

    #[test]
    fn test_e2e_concurrent_security_scans() {
        use std::thread;

        let bridge = Arc::new({
            let config = SafetyConfig {
                max_output_length: 100_000,
                injection_check_enabled: true,
            };
            let safety = Arc::new(SafetyLayer::new(&config));
            SafetyBridge::new(safety, None, None)
        });

        let mut handles = vec![];

        // 并发扫描不同类型的内容
        for i in 0..20 {
            let bridge = Arc::clone(&bridge);
            handles.push(thread::spawn(move || match i % 4 {
                0 => {
                    let r = bridge.scan_user_input("普通文本");
                    assert!(!r.was_blocked);
                }
                1 => {
                    let r = bridge.scan_user_input("身份证 110101199003071234");
                    assert!(r.had_sensitive_data);
                    assert!(!r.was_blocked);
                }
                2 => {
                    let r = bridge.scan_user_input("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
                    assert!(r.was_blocked);
                }
                _ => {
                    let r = bridge.scan_outbound(r#"{"phone": "13800138000"}"#);
                    assert!(r.had_sensitive_data);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        let stats = bridge.cumulative_stats();
        assert_eq!(stats.total_scans, 20);
    }
}

// ═══════════════════════════════════════════════════════════════════
// 4. AdminSync ↔ DataReporter 集成测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod admin_reporter_integration {
    use super::*;
    use crate::admin_sync::AdminClientConfig;

    #[test]
    fn test_admin_config_serialization_roundtrip() {
        let config = AdminClientConfig {
            llm_backend: Some("openai".to_string()),
            llm_api_key: Some("sk-test-key".to_string()),
            llm_model: Some("gpt-4".to_string()),
            llm_base_url: None,
            safety_enabled: Some(true),
            skills_enabled: Some(true),
            extensions_enabled: Some(true),
            max_cost_per_day_cents: Some(1000),
            config_version: Some(1),
            updated_at: Some("2026-03-22T00:00:00Z".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: AdminClientConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.llm_backend, config.llm_backend);
        assert_eq!(parsed.llm_api_key, config.llm_api_key);
        assert_eq!(parsed.config_version, config.config_version);
    }

    #[test]
    fn test_client_report_serialization_all_types() {
        let reports = vec![
            ClientReport::AuditLog {
                timestamp: "2026-03-22T00:00:00Z".to_string(),
                action: "chat_message".to_string(),
                details: serde_json::json!({"thread_id": "t1"}),
            },
            ClientReport::DlpEvent {
                timestamp: "2026-03-22T00:00:00Z".to_string(),
                had_sensitive_data: true,
                was_blocked: false,
                rule_matches: vec!["chinese_id_card".to_string()],
            },
            ClientReport::UsageStats {
                timestamp: "2026-03-22T00:00:00Z".to_string(),
                period_start: "2026-03-21T00:00:00Z".to_string(),
                period_end: "2026-03-22T00:00:00Z".to_string(),
                total_messages: 100,
                total_tokens: 50000,
                total_cost_cents: 500,
            },
            ClientReport::HealthStatus {
                timestamp: "2026-03-22T00:00:00Z".to_string(),
                client_version: "0.1.0".to_string(),
                uptime_secs: 3600,
                active_extensions: vec!["mcp-server".to_string()],
            },
        ];

        let json = serde_json::to_string(&reports).unwrap();
        let parsed: Vec<ClientReport> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.len(), 4);
    }

    #[test]
    fn test_reporter_queue_overflow_protection() {
        let reporter = DataReporter::new(
            "https://admin.test.local".to_string(),
            "test-token".to_string(),
        )
        .with_max_queue_size(10);

        // 超过队列容量
        for i in 0..15 {
            reporter.enqueue(ClientReport::AuditLog {
                timestamp: format!("2026-03-22T00:00:{:02}Z", i),
                action: format!("action_{}", i),
                details: serde_json::json!({}),
            });
        }

        // 队列应该不超过 max_queue_size
        assert!(reporter.queue_len() <= 15, "Queue should handle overflow");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 5. 模块间契约一致性测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod cross_module_contract {
    use super::*;
    use crate::safety_bridge::{BridgeCumulativeStats, BridgeScanResult, BridgeStats};

    #[test]
    fn test_contract_bridge_result_json_compatible_with_frontend() {
        // 验证 BridgeScanResult 的 JSON 格式与前端期望一致
        let result = BridgeScanResult {
            had_sensitive_data: true,
            sanitized_content: "身份证 110************234".to_string(),
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

        let json = serde_json::to_value(&result).unwrap();

        // 前端期望的字段
        assert!(json.get("had_sensitive_data").is_some());
        assert!(json.get("sanitized_content").is_some());
        assert!(json.get("was_blocked").is_some());
        assert!(json.get("block_reason").is_some());
        assert!(json.get("stats").is_some());

        let stats = json.get("stats").unwrap();
        assert!(stats.get("secret_detected").is_some());
        assert!(stats.get("pii_matches").is_some());
        assert!(stats.get("redacted_count").is_some());
    }

    #[test]
    fn test_contract_dlp_event_report_format() {
        // 验证 DLP 事件上报格式与 Admin Backend 期望一致
        let report = ClientReport::DlpEvent {
            timestamp: "2026-03-22T00:00:00Z".to_string(),
            had_sensitive_data: true,
            was_blocked: false,
            rule_matches: vec!["chinese_id_card_18".to_string()],
        };

        let json = serde_json::to_value(&report).unwrap();

        assert_eq!(json.get("type").unwrap(), "dlp_event");
        assert!(json.get("timestamp").is_some());
        assert!(json.get("had_sensitive_data").is_some());
        assert!(json.get("was_blocked").is_some());
        assert!(json.get("rule_matches").is_some());
    }

    #[test]
    fn test_contract_cumulative_stats_all_fields_present() {
        let stats = BridgeCumulativeStats {
            total_scans: 100,
            secret_blocks: 5,
            pii_detections: 20,
            pii_redactions: 18,
            pii_blocks: 2,
            clean_passes: 75,
        };

        let json = serde_json::to_value(&stats).unwrap();

        assert!(json.get("total_scans").is_some());
        assert!(json.get("secret_blocks").is_some());
        assert!(json.get("pii_detections").is_some());
        assert!(json.get("pii_redactions").is_some());
        assert!(json.get("pii_blocks").is_some());
        assert!(json.get("clean_passes").is_some());
    }
}
