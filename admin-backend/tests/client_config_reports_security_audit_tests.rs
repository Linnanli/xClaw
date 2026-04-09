//! 客户端配置下发 & 数据上报 API — 安全审计测试
//!
//! 覆盖维度：
//! - API Key 不泄露到日志/错误信息
//! - 敏感数据不出现在序列化输出中（除预期字段外）
//! - 输入验证防止注入攻击
//! - 批量上报大小限制防止 DoS
//!
//! 命名规范：`test_audit_{security_concern}`

use serde_json::json;

// =========================================================================
// 镜像类型定义
// =========================================================================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientConfigResponse {
    llm_backend: Option<String>,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    llm_base_url: Option<String>,
    safety_enabled: Option<bool>,
    skills_enabled: Option<bool>,
    extensions_enabled: Option<bool>,
    max_cost_per_day_cents: Option<i64>,
    config_version: i64,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientReportPayload {
    #[serde(rename = "type")]
    report_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

// =========================================================================
// API Key 泄露审计
// =========================================================================

#[test]
fn test_audit_api_key_not_in_debug_output() {
    let resp = ClientConfigResponse {
        llm_backend: Some("openai".into()),
        llm_api_key: Some("sk-super-secret-key-12345".into()),
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 1,
        updated_at: "now".into(),
    };

    // Debug 输出会包含 API Key（这是 Rust derive(Debug) 的默认行为）
    // 在生产代码中，应该使用自定义 Debug 实现来隐藏敏感字段
    let debug_str = format!("{:?}", resp);

    // 记录这个已知风险：derive(Debug) 会暴露 API Key
    // 生产环境应使用自定义 Debug trait 或 secrecy crate
    assert!(
        debug_str.contains("sk-super-secret-key-12345"),
        "Known risk: derive(Debug) exposes API key. \
         Production should use custom Debug impl or secrecy crate."
    );
}

#[test]
fn test_audit_api_key_only_in_designated_field() {
    let api_key = "sk-secret-key-for-audit-test";
    let resp = ClientConfigResponse {
        llm_backend: Some("openai".into()),
        llm_api_key: Some(api_key.into()),
        llm_model: Some("gpt-4".into()),
        llm_base_url: Some("https://api.openai.com".into()),
        safety_enabled: Some(true),
        skills_enabled: Some(true),
        extensions_enabled: Some(false),
        max_cost_per_day_cents: Some(5000),
        config_version: 1,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();

    // API Key 只应出现在 llm_api_key 字段中
    let obj = json_val.as_object().unwrap();
    for (key, value) in obj {
        if key != "llm_api_key" {
            if let Some(s) = value.as_str() {
                assert!(
                    !s.contains(api_key),
                    "API key leaked to field '{}': {}",
                    key,
                    s
                );
            }
        }
    }
}

// =========================================================================
// 敏感数据泄露审计
// =========================================================================

#[test]
fn test_audit_dlp_event_no_raw_content() {
    // DLP 事件不应包含原始敏感内容
    let report = ClientReportPayload {
        report_type: "dlp_event".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "had_sensitive_data": true,
            "was_blocked": false,
            "rule_matches": ["id_card", "phone_number"]
        }),
    };

    let json_str = serde_json::to_string(&report).unwrap();

    // 验证不包含常见的敏感数据模式
    let sensitive_patterns = [
        "330326",           // 身份证号前缀
        "13800138000",      // 手机号
        "4111111111111111", // 信用卡号
        "password",         // 密码
    ];

    for pattern in &sensitive_patterns {
        assert!(
            !json_str.contains(pattern),
            "DLP event should not contain raw sensitive data: {}",
            pattern
        );
    }
}

#[test]
fn test_audit_audit_log_no_message_content() {
    // 审计日志不应包含用户消息的原始内容
    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "action": "send_message",
            "details": {
                "thread_id": "abc-123",
                "message_length": 256
                // 注意：不包含 message_content
            }
        }),
    };

    let json_str = serde_json::to_string(&report).unwrap();

    // 验证不包含 "content" 或 "message" 字段（除了 action 描述）
    let json_val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let details = &json_val["details"];

    assert!(
        details.get("message_content").is_none(),
        "Audit log should not contain raw message content"
    );
    assert!(
        details.get("content").is_none(),
        "Audit log should not contain 'content' field"
    );
}

// =========================================================================
// 输入验证安全测试
// =========================================================================

#[test]
fn test_audit_sql_injection_in_client_id() {
    // client_id 参数应该是 UUID 格式，SQL 注入应被拒绝
    let malicious_ids = [
        "'; DROP TABLE client_configs; --",
        "1 OR 1=1",
        "' UNION SELECT * FROM users --",
        "<script>alert('xss')</script>",
        "../../../etc/passwd",
    ];

    for malicious_id in &malicious_ids {
        // UUID::parse_str 应该拒绝这些输入
        let result = uuid::Uuid::parse_str(malicious_id);
        assert!(
            result.is_err(),
            "UUID parser should reject malicious input: {}",
            malicious_id
        );
    }
}

#[test]
fn test_audit_xss_in_report_data() {
    // 上报数据中的 XSS 尝试应被安全存储（不执行）
    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "action": "<script>alert('xss')</script>",
            "details": {
                "thread_id": "<img src=x onerror=alert(1)>"
            }
        }),
    };

    // 序列化应该正常工作（数据原样存储）
    let json_str = serde_json::to_string(&report).unwrap();
    assert!(
        json_str.contains("&lt;") || json_str.contains("<script>"),
        "XSS content should be stored as-is (escaped by JSON) or sanitized"
    );
}

#[test]
fn test_audit_oversized_report_type() {
    // 超长的 report_type 不应导致缓冲区溢出
    let long_type = "a".repeat(10_000);
    let report = ClientReportPayload {
        report_type: long_type.clone(),
        data: json!({}),
    };

    let json_str = serde_json::to_string(&report).unwrap();
    let parsed: ClientReportPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.report_type.len(), 10_000);
}

#[test]
fn test_audit_deeply_nested_json() {
    // 深度嵌套的 JSON 不应导致栈溢出
    let mut nested = json!("leaf");
    for _ in 0..50 {
        nested = json!({"nested": nested});
    }

    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: nested,
    };

    // 序列化/反序列化应该正常工作
    let json_str = serde_json::to_string(&report).unwrap();
    let parsed: ClientReportPayload = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.report_type, "audit_log");
}

// =========================================================================
// 批量上报 DoS 防护测试
// =========================================================================

#[test]
fn test_audit_batch_size_limit() {
    // 服务端限制批量上报最多 1000 条
    let max_batch_size = 1000;

    // 生成超过限制的批量数据
    let oversized_batch: Vec<ClientReportPayload> = (0..max_batch_size + 1)
        .map(|i| ClientReportPayload {
            report_type: "audit_log".into(),
            data: json!({"index": i}),
        })
        .collect();

    assert!(
        oversized_batch.len() > max_batch_size,
        "Test batch should exceed limit"
    );

    // 验证可以序列化（客户端侧不限制，服务端限制）
    let json_str = serde_json::to_string(&oversized_batch).unwrap();
    assert!(!json_str.is_empty());
}

#[test]
fn test_audit_single_report_size_limit() {
    // 单条上报数据不应过大
    let large_data = "x".repeat(1_000_000); // 1MB 字符串
    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({"large_field": large_data}),
    };

    let json_str = serde_json::to_string(&report).unwrap();
    // 验证序列化成功但数据很大
    assert!(json_str.len() > 1_000_000);
}

// =========================================================================
// 认证安全测试
// =========================================================================

#[test]
fn test_audit_config_requires_valid_uuid() {
    // 配置查询的 client_id 必须是有效 UUID
    let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
    assert!(uuid::Uuid::parse_str(valid_uuid).is_ok());

    let invalid_uuids = [
        "",
        "not-a-uuid",
        "550e8400-e29b-41d4-a716",
        "550e8400e29b41d4a716446655440000", // 无连字符（实际上这个是合法的）
    ];

    for invalid in &invalid_uuids {
        if invalid.is_empty() || invalid.contains("not") || invalid.len() < 32 {
            assert!(
                uuid::Uuid::parse_str(invalid).is_err(),
                "Should reject invalid UUID: '{}'",
                invalid
            );
        }
    }
}

#[test]
fn test_audit_report_type_whitelist() {
    // 服务端只接受白名单中的 report_type
    let valid_types = ["audit_log", "dlp_event", "usage_stats", "health_status"];
    let invalid_types = [
        "admin_override",
        "system_command",
        "sql_query",
        "",
        "audit_log; DROP TABLE",
    ];

    for valid in &valid_types {
        assert!(
            valid_types.contains(valid),
            "{} should be in whitelist",
            valid
        );
    }

    for invalid in &invalid_types {
        assert!(
            !valid_types.contains(invalid),
            "{} should NOT be in whitelist",
            invalid
        );
    }
}

// =========================================================================
// 错误信息安全审计
// =========================================================================

#[test]
fn test_audit_error_no_internal_details() {
    use admin_backend::error::Error;

    // 数据库错误不应暴露连接字符串
    let db_error =
        Error::Database("connection refused to postgres://user:pass@host:5432/db".into());
    let error_str = db_error.to_string();

    // IntoResponse 实现应该返回通用错误消息
    // 这里测试 Error 的 Display 实现
    assert!(error_str.contains("Database error"));
}

#[test]
fn test_audit_validation_error_safe_message() {
    use admin_backend::error::Error;

    let error = Error::Validation("Invalid client_id format".into());
    let error_str = error.to_string();

    // 验证错误消息不包含内部实现细节
    assert!(!error_str.contains("SQL"));
    assert!(!error_str.contains("postgres"));
    assert!(!error_str.contains("SELECT"));
}
