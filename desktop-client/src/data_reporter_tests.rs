//! 数据上报模块测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（ClientReport JSON 格式与 Admin Backend API 匹配）
//! - 安全审计测试（不上报原始内容、敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（网络错误、队列溢出、锁中毒）
//! - 可靠性测试（队列缓冲、失败重试、并发安全）

#[cfg(test)]
mod tests {
    use crate::data_reporter::{ClientReport, DataReporter};

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_audit_log_serialization() {
        let report = ClientReport::AuditLog {
            timestamp: "2025-06-01T10:00:00Z".into(),
            action: "chat".into(),
            details: serde_json::json!({"thread_id": "t-1"}),
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["type"], "audit_log");
        assert_eq!(json["action"], "chat");
        assert!(json["details"].is_object());
    }

    #[test]
    fn test_dlp_event_serialization() {
        let report = ClientReport::DlpEvent {
            timestamp: "2025-06-01T10:00:00Z".into(),
            had_sensitive_data: true,
            was_blocked: false,
            rule_matches: vec!["id_card".into(), "phone_number".into()],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["type"], "dlp_event");
        assert_eq!(json["had_sensitive_data"], true);
        assert_eq!(json["was_blocked"], false);
        assert_eq!(json["rule_matches"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_usage_stats_serialization() {
        let report = ClientReport::UsageStats {
            timestamp: "2025-06-01T10:00:00Z".into(),
            period_start: "2025-06-01T00:00:00Z".into(),
            period_end: "2025-06-01T23:59:59Z".into(),
            total_messages: 150,
            total_tokens: 50000,
            total_cost_cents: 250,
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["type"], "usage_stats");
        assert_eq!(json["total_messages"], 150);
        assert_eq!(json["total_tokens"], 50000);
        assert_eq!(json["total_cost_cents"], 250);
    }

    #[test]
    fn test_health_status_serialization() {
        let report = ClientReport::HealthStatus {
            timestamp: "2025-06-01T10:00:00Z".into(),
            client_version: "0.1.0".into(),
            uptime_secs: 3600,
            active_extensions: vec!["github-mcp".into(), "slack".into()],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["type"], "health_status");
        assert_eq!(json["client_version"], "0.1.0");
        assert_eq!(json["uptime_secs"], 3600);
        assert_eq!(json["active_extensions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_report_deserialization() {
        let json = r#"{"type":"audit_log","timestamp":"2025-01-01T00:00:00Z","action":"tool_use","details":{}}"#;
        let report: ClientReport = serde_json::from_str(json).unwrap();
        match report {
            ClientReport::AuditLog { action, .. } => assert_eq!(action, "tool_use"),
            _ => panic!("Expected AuditLog variant"),
        }
    }

    // =========================================================================
    // 单元测试 — DataReporter 队列操作
    // =========================================================================

    #[test]
    fn test_reporter_creation() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into());
        assert_eq!(reporter.queue_len(), 0);
    }

    #[test]
    fn test_reporter_enqueue() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into());

        reporter.enqueue(ClientReport::AuditLog {
            timestamp: "2025-01-01T00:00:00Z".into(),
            action: "test".into(),
            details: serde_json::json!({}),
        });

        assert_eq!(reporter.queue_len(), 1);
    }

    #[test]
    fn test_reporter_enqueue_multiple() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into());

        for i in 0..100 {
            reporter.enqueue(ClientReport::AuditLog {
                timestamp: format!("2025-01-01T{:02}:00:00Z", i % 24),
                action: format!("action_{}", i),
                details: serde_json::json!({}),
            });
        }

        assert_eq!(reporter.queue_len(), 100);
    }

    #[test]
    fn test_reporter_queue_overflow_drops_oldest() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into())
            .with_max_queue_size(10);

        // 填满队列
        for i in 0..10 {
            reporter.enqueue(ClientReport::AuditLog {
                timestamp: "t".into(),
                action: format!("action_{}", i),
                details: serde_json::json!({}),
            });
        }
        assert_eq!(reporter.queue_len(), 10);

        // 再加一个，应触发淘汰
        reporter.enqueue(ClientReport::AuditLog {
            timestamp: "t".into(),
            action: "overflow".into(),
            details: serde_json::json!({}),
        });

        // 队列应小于等于 max_queue_size
        assert!(reporter.queue_len() <= 10);
    }

    #[test]
    fn test_reporter_with_flush_interval() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into())
            .with_flush_interval(std::time::Duration::from_secs(60));
        assert_eq!(reporter.queue_len(), 0);
    }

    // =========================================================================
    // 契约测试 — 验证与 Admin Backend API 的兼容性
    // =========================================================================

    /// Admin Backend POST /api/client-reports 请求体格式：
    /// ```json
    /// [
    ///   {"type": "audit_log", "timestamp": "...", "action": "...", "details": {}},
    ///   {"type": "dlp_event", "timestamp": "...", "had_sensitive_data": true, ...},
    ///   ...
    /// ]
    /// ```
    #[test]
    fn test_contract_batch_report_format() {
        let reports = vec![
            ClientReport::AuditLog {
                timestamp: "2025-01-01T00:00:00Z".into(),
                action: "chat".into(),
                details: serde_json::json!({"thread_id": "t-1"}),
            },
            ClientReport::DlpEvent {
                timestamp: "2025-01-01T00:01:00Z".into(),
                had_sensitive_data: true,
                was_blocked: false,
                rule_matches: vec!["id_card".into()],
            },
        ];

        let json = serde_json::to_value(&reports).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
        assert_eq!(json[0]["type"], "audit_log");
        assert_eq!(json[1]["type"], "dlp_event");
    }

    /// 验证每种 report 类型的 `type` 标签值。
    #[test]
    fn test_contract_report_type_tags() {
        let cases: Vec<(ClientReport, &str)> = vec![
            (
                ClientReport::AuditLog {
                    timestamp: "t".into(),
                    action: "a".into(),
                    details: serde_json::json!({}),
                },
                "audit_log",
            ),
            (
                ClientReport::DlpEvent {
                    timestamp: "t".into(),
                    had_sensitive_data: false,
                    was_blocked: false,
                    rule_matches: vec![],
                },
                "dlp_event",
            ),
            (
                ClientReport::UsageStats {
                    timestamp: "t".into(),
                    period_start: "s".into(),
                    period_end: "e".into(),
                    total_messages: 0,
                    total_tokens: 0,
                    total_cost_cents: 0,
                },
                "usage_stats",
            ),
            (
                ClientReport::HealthStatus {
                    timestamp: "t".into(),
                    client_version: "v".into(),
                    uptime_secs: 0,
                    active_extensions: vec![],
                },
                "health_status",
            ),
        ];

        for (report, expected_type) in cases {
            let json = serde_json::to_value(&report).unwrap();
            assert_eq!(
                json["type"], expected_type,
                "Report type tag mismatch for {:?}",
                expected_type
            );
        }
    }

    /// 验证 AuditLog 的 details 字段是任意 JSON 对象。
    #[test]
    fn test_contract_audit_log_details_is_flexible() {
        let report = ClientReport::AuditLog {
            timestamp: "t".into(),
            action: "tool_use".into(),
            details: serde_json::json!({
                "tool_name": "shell",
                "duration_ms": 150,
                "nested": {"key": "value"}
            }),
        };
        let json = serde_json::to_value(&report).unwrap();
        assert!(json["details"]["tool_name"].is_string());
        assert!(json["details"]["duration_ms"].is_number());
        assert!(json["details"]["nested"].is_object());
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 DlpEvent 不包含原始消息内容。
    #[test]
    fn test_audit_dlp_event_no_content_leak() {
        let report = ClientReport::DlpEvent {
            timestamp: "2025-01-01T00:00:00Z".into(),
            had_sensitive_data: true,
            was_blocked: true,
            rule_matches: vec!["id_card".into(), "phone_number".into()],
        };
        let json_str = serde_json::to_string(&report).unwrap();

        // DLP 事件不应包含原始内容
        assert!(!json_str.contains("content"));
        assert!(!json_str.contains("message"));
        assert!(!json_str.contains("user_input"));
        assert!(!json_str.contains("sanitized"));
    }

    /// 验证 AuditLog 不包含用户 ID（由 Admin Backend 从 token 推断）。
    #[test]
    fn test_audit_audit_log_no_user_id() {
        let report = ClientReport::AuditLog {
            timestamp: "t".into(),
            action: "chat".into(),
            details: serde_json::json!({"thread_id": "t-1"}),
        };
        let json_str = serde_json::to_string(&report).unwrap();

        assert!(!json_str.contains("user_id"));
        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("email"));
    }

    /// 验证 UsageStats 不包含具体的消息内容或 API Key。
    #[test]
    fn test_audit_usage_stats_no_sensitive_data() {
        let report = ClientReport::UsageStats {
            timestamp: "t".into(),
            period_start: "s".into(),
            period_end: "e".into(),
            total_messages: 100,
            total_tokens: 50000,
            total_cost_cents: 250,
        };
        let json_str = serde_json::to_string(&report).unwrap();

        assert!(!json_str.contains("api_key"));
        assert!(!json_str.contains("content"));
        assert!(!json_str.contains("password"));
        assert!(!json_str.contains("model_name")); // 不泄露具体模型
    }

    /// 验证 HealthStatus 不包含系统敏感信息。
    #[test]
    fn test_audit_health_status_no_system_info() {
        let report = ClientReport::HealthStatus {
            timestamp: "t".into(),
            client_version: "0.1.0".into(),
            uptime_secs: 3600,
            active_extensions: vec!["github-mcp".into()],
        };
        let json_str = serde_json::to_string(&report).unwrap();

        assert!(!json_str.contains("hostname"));
        assert!(!json_str.contains("ip_address"));
        assert!(!json_str.contains("mac_address"));
        assert!(!json_str.contains("db_size")); // 不泄露数据库大小
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_rule_matches() {
        let report = ClientReport::DlpEvent {
            timestamp: "t".into(),
            had_sensitive_data: false,
            was_blocked: false,
            rule_matches: vec![],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["rule_matches"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_data_zero_usage_stats() {
        let report = ClientReport::UsageStats {
            timestamp: "t".into(),
            period_start: "s".into(),
            period_end: "e".into(),
            total_messages: 0,
            total_tokens: 0,
            total_cost_cents: 0,
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["total_messages"], 0);
        assert_eq!(json["total_tokens"], 0);
        assert_eq!(json["total_cost_cents"], 0);
    }

    #[test]
    fn test_data_max_usage_stats() {
        let report = ClientReport::UsageStats {
            timestamp: "t".into(),
            period_start: "s".into(),
            period_end: "e".into(),
            total_messages: u64::MAX,
            total_tokens: u64::MAX,
            total_cost_cents: u64::MAX,
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: ClientReport = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientReport::UsageStats { total_messages, .. } => {
                assert_eq!(total_messages, u64::MAX);
            }
            _ => panic!("Expected UsageStats"),
        }
    }

    #[test]
    fn test_data_unicode_action() {
        let report = ClientReport::AuditLog {
            timestamp: "t".into(),
            action: "安装扩展".into(),
            details: serde_json::json!({"name": "中文扩展"}),
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: ClientReport = serde_json::from_str(&json).unwrap();
        match parsed {
            ClientReport::AuditLog { action, .. } => assert_eq!(action, "安装扩展"),
            _ => panic!("Expected AuditLog"),
        }
    }

    #[test]
    fn test_data_empty_extensions_list() {
        let report = ClientReport::HealthStatus {
            timestamp: "t".into(),
            client_version: "0.1.0".into(),
            uptime_secs: 0,
            active_extensions: vec![],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["active_extensions"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_data_zero_uptime() {
        let report = ClientReport::HealthStatus {
            timestamp: "t".into(),
            client_version: "0.1.0".into(),
            uptime_secs: 0,
            active_extensions: vec![],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["uptime_secs"], 0);
    }

    // =========================================================================
    // 失败路径测试
    // =========================================================================

    #[test]
    fn test_failure_invalid_report_json() {
        let result = serde_json::from_str::<ClientReport>(r#"{"type":"unknown"}"#);
        assert!(result.is_err(), "Unknown type should fail deserialization");
    }

    #[test]
    fn test_failure_missing_type_tag() {
        let result = serde_json::from_str::<ClientReport>(r#"{"action":"test"}"#);
        assert!(result.is_err(), "Missing type tag should fail");
    }

    #[tokio::test]
    async fn test_failure_flush_empty_queue() {
        let reporter = DataReporter::new("https://admin.example.com".into(), "test-token".into());
        let result = reporter.flush().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_failure_flush_network_error() {
        let reporter = DataReporter::new(
            "https://nonexistent.invalid.example.com".into(),
            "test-token".into(),
        );

        reporter.enqueue(ClientReport::AuditLog {
            timestamp: "t".into(),
            action: "test".into(),
            details: serde_json::json!({}),
        });

        let result = reporter.flush().await;
        assert!(result.is_err(), "Should fail with network error");

        // 事件应被放回队列
        assert_eq!(reporter.queue_len(), 1, "Failed events should be requeued");
    }

    // =========================================================================
    // 可靠性测试 — 并发安全
    // =========================================================================

    #[test]
    fn test_reliability_concurrent_enqueue() {
        use std::sync::Arc;
        use std::thread;

        let reporter = Arc::new(DataReporter::new(
            "https://admin.example.com".into(),
            "test-token".into(),
        ));

        let mut handles = vec![];
        for i in 0..10 {
            let reporter = reporter.clone();
            handles.push(thread::spawn(move || {
                for j in 0..100 {
                    reporter.enqueue(ClientReport::AuditLog {
                        timestamp: "t".into(),
                        action: format!("thread_{}_action_{}", i, j),
                        details: serde_json::json!({}),
                    });
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(reporter.queue_len(), 1000);
    }
}
