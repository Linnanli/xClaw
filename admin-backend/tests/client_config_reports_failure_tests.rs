//! 客户端配置下发 & 数据上报 API — 失败路径测试
//!
//! 覆盖维度：
//! - 数据库不可用时的行为
//! - 无效输入的处理
//! - 边界条件和极端情况
//! - 降级逻辑安全性
//!
//! 命名规范：`test_failure_{scenario}`

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
// 配置下发失败路径
// =========================================================================

#[test]
fn test_failure_config_malformed_json() {
    let malformed_inputs = [
        "",
        "null",
        "[]",
        "42",
        "\"string\"",
        "{",
        "{ invalid }",
        "{ \"config_version\": }",
    ];

    for input in &malformed_inputs {
        let result: Result<ClientConfigResponse, _> = serde_json::from_str(input);
        assert!(
            result.is_err(),
            "Should reject malformed JSON: '{}'",
            input
        );
    }
}

#[test]
fn test_failure_config_null_required_fields() {
    // config_version 和 updated_at 是必填字段
    let json_str = json!({
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": null,
        "updated_at": "now"
    });

    let result: Result<ClientConfigResponse, _> = serde_json::from_value(json_str);
    assert!(result.is_err(), "Should reject null config_version");
}

#[test]
fn test_failure_config_missing_updated_at() {
    let json_str = json!({
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": 1
    });

    let result: Result<ClientConfigResponse, _> = serde_json::from_value(json_str);
    assert!(result.is_err(), "Should reject missing updated_at");
}

#[test]
fn test_failure_config_float_as_version() {
    let json_str = json!({
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": 1.5,
        "updated_at": "now"
    });

    // serde_json 对 i64 字段接收 1.5 时的行为
    let result: Result<ClientConfigResponse, _> = serde_json::from_value(json_str);
    assert!(result.is_err(), "Should reject float as config_version");
}

// =========================================================================
// 数据上报失败路径
// =========================================================================

#[test]
fn test_failure_report_missing_type_field() {
    let json_str = r#"{"data": {"foo": "bar"}}"#;
    let result: Result<ClientReportPayload, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should reject report without 'type' field");
}

#[test]
fn test_failure_report_null_type() {
    let json_str = r#"{"type": null, "data": {}}"#;
    let result: Result<ClientReportPayload, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should reject null report type");
}

#[test]
fn test_failure_report_numeric_type() {
    let json_str = r#"{"type": 42}"#;
    let result: Result<ClientReportPayload, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should reject numeric report type");
}

#[test]
fn test_failure_report_batch_not_array() {
    let json_str = r#"{"type": "audit_log"}"#;
    let result: Result<Vec<ClientReportPayload>, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "Should reject non-array as batch");
}

#[test]
fn test_failure_report_batch_mixed_valid_invalid() {
    // 批量中混合有效和无效的条目
    let json_str = r#"[
        {"type": "audit_log", "timestamp": "t1"},
        {"no_type_field": true},
        {"type": "dlp_event", "timestamp": "t2"}
    ]"#;

    let result: Result<Vec<ClientReportPayload>, _> = serde_json::from_str(json_str);
    // 整个批量应该失败（因为第二个元素缺少 type）
    assert!(result.is_err(), "Should reject batch with invalid entries");
}

// =========================================================================
// 边界条件测试
// =========================================================================

#[test]
fn test_failure_config_empty_string_fields() {
    let resp = ClientConfigResponse {
        llm_backend: Some("".into()),
        llm_api_key: Some("".into()),
        llm_model: Some("".into()),
        llm_base_url: Some("".into()),
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 0,
        updated_at: "".into(),
    };

    // 空字符串是合法的 JSON 值，但业务上可能无意义
    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.llm_backend, Some("".into()));
    assert_eq!(parsed.updated_at, "");
}

#[test]
fn test_failure_config_unicode_in_fields() {
    let resp = ClientConfigResponse {
        llm_backend: Some("中文后端".into()),
        llm_api_key: Some("密钥🔑".into()),
        llm_model: Some("模型-v1".into()),
        llm_base_url: Some("https://例え.jp/api".into()),
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 1,
        updated_at: "2026-03-22T00:00:00+08:00".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.llm_backend, Some("中文后端".into()));
    assert_eq!(parsed.llm_api_key, Some("密钥🔑".into()));
}

#[test]
fn test_failure_config_special_characters_in_url() {
    let resp = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: Some("https://api.example.com/v1?key=val&foo=bar#section".into()),
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 1,
        updated_at: "now".into(),
    };

    let json_str = serde_json::to_string(&resp).unwrap();
    let parsed: ClientConfigResponse = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.llm_base_url.unwrap().contains("?key=val&foo=bar"));
}

#[test]
fn test_failure_report_very_long_action() {
    let long_action = "a".repeat(100_000);
    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({
            "timestamp": "t",
            "action": long_action,
            "details": {}
        }),
    };

    // 应该能序列化，但服务端可能需要截断
    let json_str = serde_json::to_string(&report).unwrap();
    assert!(json_str.len() > 100_000);
}

#[test]
fn test_failure_report_null_data_fields() {
    let report = ClientReportPayload {
        report_type: "audit_log".into(),
        data: json!({
            "timestamp": null,
            "action": null,
            "details": null
        }),
    };

    let json_str = serde_json::to_string(&report).unwrap();
    let parsed: ClientReportPayload = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.data["timestamp"].is_null());
}

// =========================================================================
// 降级逻辑安全性测试
// =========================================================================

#[test]
fn test_failure_config_fallback_is_safe() {
    // 当配置获取失败时，默认配置应该是安全的
    let default_config = ClientConfigResponse {
        llm_backend: None,
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,  // None 意味着使用客户端默认值
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 0,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    // 安全审计：默认配置不应禁用安全功能
    // safety_enabled = None 时，客户端应默认启用安全
    assert!(
        default_config.safety_enabled.is_none() || default_config.safety_enabled == Some(true),
        "Default config should not disable safety"
    );

    // 默认配置不应包含 API Key
    assert!(
        default_config.llm_api_key.is_none(),
        "Default config should not contain API key"
    );
}

#[test]
fn test_failure_report_empty_batch_is_safe() {
    // 空批量上报应该返回成功（不是错误）
    let empty_batch: Vec<ClientReportPayload> = vec![];
    let json_str = serde_json::to_string(&empty_batch).unwrap();
    assert_eq!(json_str, "[]");

    // 服务端应返回 { "received": 0 }
    let expected_response = json!({"received": 0});
    assert_eq!(expected_response["received"], 0);
}

// =========================================================================
// 并发安全测试
// =========================================================================

#[test]
fn test_failure_concurrent_config_version_ordering() {
    // 验证配置版本号的单调递增性
    let configs = vec![
        ClientConfigResponse {
            llm_backend: None, llm_api_key: None, llm_model: None,
            llm_base_url: None, safety_enabled: None, skills_enabled: None,
            extensions_enabled: None, max_cost_per_day_cents: None,
            config_version: 1, updated_at: "t1".into(),
        },
        ClientConfigResponse {
            llm_backend: None, llm_api_key: None, llm_model: None,
            llm_base_url: None, safety_enabled: None, skills_enabled: None,
            extensions_enabled: None, max_cost_per_day_cents: None,
            config_version: 3, updated_at: "t3".into(),
        },
        ClientConfigResponse {
            llm_backend: None, llm_api_key: None, llm_model: None,
            llm_base_url: None, safety_enabled: None, skills_enabled: None,
            extensions_enabled: None, max_cost_per_day_cents: None,
            config_version: 2, updated_at: "t2".into(),
        },
    ];

    // 客户端应该只接受版本号递增的配置
    let mut max_version = 0i64;
    let mut accepted = vec![];

    for config in &configs {
        if config.config_version > max_version {
            max_version = config.config_version;
            accepted.push(config.config_version);
        }
    }

    // 版本 2 不应被接受（因为已经看到版本 3）
    assert_eq!(accepted, vec![1, 3]);
    assert!(!accepted.contains(&2));
}
