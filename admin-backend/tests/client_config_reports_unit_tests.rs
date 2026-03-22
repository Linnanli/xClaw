//! 客户端配置下发 & 数据上报 API — 单元测试
//!
//! 覆盖维度：
//! - 正常路径：序列化/反序列化、默认值、边界值
//! - 错误路径：无效输入、格式错误、类型不匹配
//!
//! 命名规范：`test_{module}_{scenario}`

use serde_json::json;

// =========================================================================
// ClientConfigResponse 序列化/反序列化
// =========================================================================

/// 管理端配置响应（镜像 routes.rs 中的结构体，用于独立测试）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
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

/// 客户端侧配置结构体（镜像 desktop-client 的 AdminClientConfig）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct AdminClientConfig {
    llm_backend: Option<String>,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    llm_base_url: Option<String>,
    safety_enabled: Option<bool>,
    skills_enabled: Option<bool>,
    extensions_enabled: Option<bool>,
    max_cost_per_day_cents: Option<u64>,
    config_version: Option<u64>,
    updated_at: Option<String>,
}

/// 客户端上报事件（镜像 routes.rs 中的 ClientReportPayload）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientReportPayload {
    #[serde(rename = "type")]
    report_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

// =========================================================================
// 正常路径测试
// =========================================================================

#[test]
fn test_config_response_full_serialize() {
    let resp = ClientConfigResponse {
        llm_backend: Some("openai".into()),
        llm_api_key: Some("sk-test-key".into()),
        llm_model: Some("gpt-4".into()),
        llm_base_url: Some("https://api.openai.com".into()),
        safety_enabled: Some(true),
        skills_enabled: Some(true),
        extensions_enabled: Some(false),
        max_cost_per_day_cents: Some(5000),
        config_version: 42,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(resp, parsed);
}

#[test]
fn test_config_response_all_none_fields() {
    let resp = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 0,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // None 字段应序列化为 null
    assert!(parsed["llm_backend"].is_null());
    assert!(parsed["llm_api_key"].is_null());
    assert_eq!(parsed["config_version"], 0);
}

#[test]
fn test_config_response_version_zero_is_valid() {
    let json_str = r#"{
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": 0,
        "updated_at": "2026-03-22T00:00:00Z"
    }"#;

    let parsed: ClientConfigResponse = serde_json::from_str(json_str).unwrap();
    assert_eq!(parsed.config_version, 0);
}

#[test]
fn test_config_response_large_version_number() {
    let resp = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: i64::MAX,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.config_version, i64::MAX);
}

#[test]
fn test_config_response_max_cost_boundary() {
    let resp = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: Some(0),
        config_version: 1,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.max_cost_per_day_cents, Some(0));
}

// =========================================================================
// ClientReportPayload 序列化/反序列化
// =========================================================================

#[test]
fn test_report_payload_audit_log() {
    let payload = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "action": "send_message",
            "details": { "thread_id": "abc-123" }
        }),
    };

    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("\"type\":\"audit_log\""));
    assert!(json_str.contains("send_message"));
}

#[test]
fn test_report_payload_dlp_event() {
    let payload = ClientReportPayload {
        report_type: "dlp_event".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "had_sensitive_data": true,
            "was_blocked": false,
            "rule_matches": ["id_card", "phone_number"]
        }),
    };

    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("\"type\":\"dlp_event\""));
    assert!(json_str.contains("had_sensitive_data"));
}

#[test]
fn test_report_payload_usage_stats() {
    let payload = ClientReportPayload {
        report_type: "usage_stats".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "period_start": "2026-03-21T00:00:00Z",
            "period_end": "2026-03-22T00:00:00Z",
            "total_messages": 150,
            "total_tokens": 50000,
            "total_cost_cents": 250
        }),
    };

    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("\"type\":\"usage_stats\""));
    assert!(json_str.contains("total_messages"));
}

#[test]
fn test_report_payload_health_status() {
    let payload = ClientReportPayload {
        report_type: "health_status".into(),
        data: json!({
            "timestamp": "2026-03-22T00:00:00Z",
            "client_version": "0.1.0",
            "uptime_secs": 3600,
            "active_extensions": ["web_search", "code_review"]
        }),
    };

    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("\"type\":\"health_status\""));
    assert!(json_str.contains("uptime_secs"));
}

#[test]
fn test_report_payload_batch_serialize() {
    let batch = vec![
        ClientReportPayload {
            report_type: "audit_log".into(),
            data: json!({"timestamp": "t1", "action": "a1"}),
        },
        ClientReportPayload {
            report_type: "dlp_event".into(),
            data: json!({"timestamp": "t2", "had_sensitive_data": false}),
        },
    ];

    let json_str = serde_json::to_string(&batch).unwrap();
    let parsed: Vec<ClientReportPayload> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].report_type, "audit_log");
    assert_eq!(parsed[1].report_type, "dlp_event");
}

// =========================================================================
// 错误路径测试
// =========================================================================

#[test]
fn test_config_response_missing_required_field() {
    // config_version 是必填字段
    let json_str = r#"{
        "llm_backend": null,
        "updated_at": "2026-03-22T00:00:00Z"
    }"#;

    let result: Result<ClientConfigResponse, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should fail when config_version is missing");
}

#[test]
fn test_config_response_wrong_type_for_version() {
    let json_str = r#"{
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": "not_a_number",
        "updated_at": "2026-03-22T00:00:00Z"
    }"#;

    let result: Result<ClientConfigResponse, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should fail when config_version is a string");
}

#[test]
fn test_config_response_wrong_type_for_boolean() {
    let json_str = r#"{
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": "yes",
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": 1,
        "updated_at": "2026-03-22T00:00:00Z"
    }"#;

    let result: Result<ClientConfigResponse, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should fail when safety_enabled is a string");
}

#[test]
fn test_report_payload_empty_type() {
    let payload = ClientReportPayload {
        report_type: "".into(),
        data: json!({}),
    };

    // 空类型可以序列化，但服务端应拒绝
    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("\"type\":\"\""));
}

#[test]
fn test_report_payload_unknown_type() {
    let payload = ClientReportPayload {
        report_type: "unknown_event_type".into(),
        data: json!({"foo": "bar"}),
    };

    // 未知类型可以序列化，但服务端应跳过
    let json_str = serde_json::to_string(&payload).unwrap();
    assert!(json_str.contains("unknown_event_type"));
}

#[test]
fn test_report_payload_empty_batch() {
    let batch: Vec<ClientReportPayload> = vec![];
    let json_str = serde_json::to_string(&batch).unwrap();
    assert_eq!(json_str, "[]");
}

#[test]
fn test_report_payload_invalid_json() {
    let invalid = r#"{ "type": "audit_log", "data": }"#;
    let result: Result<ClientReportPayload, _> = serde_json::from_str(invalid);
    assert!(result.is_err());
}

#[test]
fn test_config_response_negative_cost() {
    // 负数花费在 i64 范围内是合法的 JSON，但业务上无意义
    let resp = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: Some(-100),
        config_version: 1,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.max_cost_per_day_cents, Some(-100));
}
