//! 客户端配置下发 & 数据上报 API — 契约测试
//!
//! 验证 Admin Backend 的响应格式与 Desktop Client 的期望格式一致。
//! 这是防止前后端接口不匹配的关键测试。
//!
//! 覆盖维度：
//! - 字段名称一致性
//! - 类型兼容性（i64 vs u64 等）
//! - 可选字段处理
//! - 序列化/反序列化双向兼容
//!
//! 命名规范：`test_contract_{interface}_{case}`

use serde_json::json;

// =========================================================================
// 镜像类型定义
// =========================================================================

/// Admin Backend 侧的配置响应（来自 routes.rs）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ServerConfigResponse {
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

/// Desktop Client 侧的配置结构体（来自 admin_sync.rs）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientAdminConfig {
    llm_backend: Option<String>,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    llm_base_url: Option<String>,
    safety_enabled: Option<bool>,
    skills_enabled: Option<bool>,
    extensions_enabled: Option<bool>,
    /// 注意：客户端使用 u64，服务端使用 i64
    max_cost_per_day_cents: Option<u64>,
    /// 注意：客户端使用 Option<u64>，服务端使用 i64
    config_version: Option<u64>,
    updated_at: Option<String>,
}

/// Desktop Client 侧的上报事件（来自 data_reporter.rs）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
enum ClientReport {
    #[serde(rename = "audit_log")]
    AuditLog {
        timestamp: String,
        action: String,
        details: serde_json::Value,
    },
    #[serde(rename = "dlp_event")]
    DlpEvent {
        timestamp: String,
        had_sensitive_data: bool,
        was_blocked: bool,
        rule_matches: Vec<String>,
    },
    #[serde(rename = "usage_stats")]
    UsageStats {
        timestamp: String,
        period_start: String,
        period_end: String,
        total_messages: u64,
        total_tokens: u64,
        total_cost_cents: u64,
    },
    #[serde(rename = "health_status")]
    HealthStatus {
        timestamp: String,
        client_version: String,
        uptime_secs: u64,
        active_extensions: Vec<String>,
    },
}

/// Admin Backend 侧的上报接收结构体（来自 routes.rs）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ServerReportPayload {
    #[serde(rename = "type")]
    report_type: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

// =========================================================================
// 配置下发契约测试
// =========================================================================

#[test]
fn test_contract_config_server_to_client_full() {
    // 模拟服务端返回完整配置
    let server_resp = ServerConfigResponse {
        llm_backend: Some("openai".into()),
        llm_api_key: Some("sk-test".into()),
        llm_model: Some("gpt-4".into()),
        llm_base_url: Some("https://api.openai.com".into()),
        safety_enabled: Some(true),
        skills_enabled: Some(true),
        extensions_enabled: Some(false),
        max_cost_per_day_cents: Some(5000),
        config_version: 42,
        updated_at: "2026-03-22T00:00:00Z".into(),
    };

    // 序列化为 JSON（模拟 HTTP 传输）
    let json_str = serde_json::to_string(&server_resp).unwrap();

    // 客户端反序列化
    let client_config: ClientAdminConfig = serde_json::from_str(&json_str).unwrap();

    // 验证字段一致性
    assert_eq!(client_config.llm_backend, server_resp.llm_backend);
    assert_eq!(client_config.llm_api_key, server_resp.llm_api_key);
    assert_eq!(client_config.llm_model, server_resp.llm_model);
    assert_eq!(client_config.llm_base_url, server_resp.llm_base_url);
    assert_eq!(client_config.safety_enabled, server_resp.safety_enabled);
    assert_eq!(client_config.skills_enabled, server_resp.skills_enabled);
    assert_eq!(client_config.extensions_enabled, server_resp.extensions_enabled);
    assert_eq!(client_config.updated_at, Some(server_resp.updated_at));

    // 类型兼容性：i64 → u64（正数时兼容）
    assert_eq!(
        client_config.max_cost_per_day_cents,
        Some(server_resp.max_cost_per_day_cents.unwrap() as u64)
    );
    assert_eq!(
        client_config.config_version,
        Some(server_resp.config_version as u64)
    );
}

#[test]
fn test_contract_config_server_to_client_empty() {
    // 模拟服务端返回空配置（无数据库记录时）
    let server_resp = ServerConfigResponse {
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

    let json_str = serde_json::to_string(&server_resp).unwrap();
    let client_config: ClientAdminConfig = serde_json::from_str(&json_str).unwrap();

    assert!(client_config.llm_backend.is_none());
    assert!(client_config.llm_api_key.is_none());
    assert_eq!(client_config.config_version, Some(0));
}

#[test]
fn test_contract_config_field_names_match() {
    // 验证 JSON 字段名称完全一致
    let server_resp = ServerConfigResponse {
        llm_backend: Some("test".into()),
        llm_api_key: None,
        llm_model: None,
        llm_base_url: None,
        safety_enabled: None,
        skills_enabled: None,
        extensions_enabled: None,
        max_cost_per_day_cents: None,
        config_version: 1,
        updated_at: "now".into(),
    };

    let server_json: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&server_resp).unwrap()).unwrap();

    let expected_fields = [
        "llm_backend",
        "llm_api_key",
        "llm_model",
        "llm_base_url",
        "safety_enabled",
        "skills_enabled",
        "extensions_enabled",
        "max_cost_per_day_cents",
        "config_version",
        "updated_at",
    ];

    let obj = server_json.as_object().unwrap();
    for field in &expected_fields {
        assert!(
            obj.contains_key(*field),
            "Server response missing field: {}",
            field
        );
    }
}

#[test]
fn test_contract_config_extra_fields_ignored() {
    // 服务端返回额外字段时，客户端应忽略（向前兼容）
    let json_with_extra = json!({
        "llm_backend": "openai",
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": null,
        "config_version": 1,
        "updated_at": "2026-03-22T00:00:00Z",
        "future_field_1": "some_value",
        "future_field_2": 42
    });

    let result: Result<ClientAdminConfig, _> =
        serde_json::from_value(json_with_extra);
    assert!(result.is_ok(), "Client should ignore unknown fields");
}

// =========================================================================
// 数据上报契约测试
// =========================================================================

#[test]
fn test_contract_report_audit_log_client_to_server() {
    let client_report = ClientReport::AuditLog {
        timestamp: "2026-03-22T00:00:00Z".into(),
        action: "send_message".into(),
        details: json!({"thread_id": "abc-123"}),
    };

    // 客户端序列化
    let json_str = serde_json::to_string(&client_report).unwrap();

    // 服务端反序列化
    let server_payload: ServerReportPayload = serde_json::from_str(&json_str).unwrap();

    assert_eq!(server_payload.report_type, "audit_log");
    assert_eq!(server_payload.data["action"], "send_message");
    assert_eq!(server_payload.data["timestamp"], "2026-03-22T00:00:00Z");
}

#[test]
fn test_contract_report_dlp_event_client_to_server() {
    let client_report = ClientReport::DlpEvent {
        timestamp: "2026-03-22T00:00:00Z".into(),
        had_sensitive_data: true,
        was_blocked: false,
        rule_matches: vec!["id_card".into(), "phone".into()],
    };

    let json_str = serde_json::to_string(&client_report).unwrap();
    let server_payload: ServerReportPayload = serde_json::from_str(&json_str).unwrap();

    assert_eq!(server_payload.report_type, "dlp_event");
    assert_eq!(server_payload.data["had_sensitive_data"], true);
    assert_eq!(server_payload.data["was_blocked"], false);
}

#[test]
fn test_contract_report_usage_stats_client_to_server() {
    let client_report = ClientReport::UsageStats {
        timestamp: "2026-03-22T00:00:00Z".into(),
        period_start: "2026-03-21T00:00:00Z".into(),
        period_end: "2026-03-22T00:00:00Z".into(),
        total_messages: 150,
        total_tokens: 50000,
        total_cost_cents: 250,
    };

    let json_str = serde_json::to_string(&client_report).unwrap();
    let server_payload: ServerReportPayload = serde_json::from_str(&json_str).unwrap();

    assert_eq!(server_payload.report_type, "usage_stats");
    assert_eq!(server_payload.data["total_messages"], 150);
    assert_eq!(server_payload.data["total_tokens"], 50000);
}

#[test]
fn test_contract_report_health_status_client_to_server() {
    let client_report = ClientReport::HealthStatus {
        timestamp: "2026-03-22T00:00:00Z".into(),
        client_version: "0.1.0".into(),
        uptime_secs: 3600,
        active_extensions: vec!["web_search".into()],
    };

    let json_str = serde_json::to_string(&client_report).unwrap();
    let server_payload: ServerReportPayload = serde_json::from_str(&json_str).unwrap();

    assert_eq!(server_payload.report_type, "health_status");
    assert_eq!(server_payload.data["client_version"], "0.1.0");
    assert_eq!(server_payload.data["uptime_secs"], 3600);
}

#[test]
fn test_contract_report_batch_client_to_server() {
    // 客户端发送混合类型的批量上报
    let batch = vec![
        ClientReport::AuditLog {
            timestamp: "t1".into(),
            action: "a1".into(),
            details: json!({}),
        },
        ClientReport::DlpEvent {
            timestamp: "t2".into(),
            had_sensitive_data: false,
            was_blocked: false,
            rule_matches: vec![],
        },
        ClientReport::HealthStatus {
            timestamp: "t3".into(),
            client_version: "0.1.0".into(),
            uptime_secs: 100,
            active_extensions: vec![],
        },
    ];

    let json_str = serde_json::to_string(&batch).unwrap();

    // 服务端按 Vec<ServerReportPayload> 反序列化
    let server_batch: Vec<ServerReportPayload> = serde_json::from_str(&json_str).unwrap();

    assert_eq!(server_batch.len(), 3);
    assert_eq!(server_batch[0].report_type, "audit_log");
    assert_eq!(server_batch[1].report_type, "dlp_event");
    assert_eq!(server_batch[2].report_type, "health_status");
}

#[test]
fn test_contract_report_type_field_name() {
    // 验证 serde(tag = "type") 和 serde(rename = "type") 产生相同的 JSON 字段名
    let client_report = ClientReport::AuditLog {
        timestamp: "t".into(),
        action: "a".into(),
        details: json!({}),
    };

    let json_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&client_report).unwrap()).unwrap();

    // 客户端使用 #[serde(tag = "type")]，字段名应为 "type"
    assert!(json_val.get("type").is_some(), "Missing 'type' field");
    assert_eq!(json_val["type"], "audit_log");
}

// =========================================================================
// 类型兼容性边界测试
// =========================================================================

#[test]
fn test_contract_config_i64_to_u64_positive() {
    // 服务端 i64 正数 → 客户端 u64 应兼容
    let server_json = json!({
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": 9999,
        "config_version": 100,
        "updated_at": "now"
    });

    let client: ClientAdminConfig = serde_json::from_value(server_json).unwrap();
    assert_eq!(client.max_cost_per_day_cents, Some(9999));
    assert_eq!(client.config_version, Some(100));
}

#[test]
fn test_contract_config_zero_values() {
    let server_json = json!({
        "llm_backend": null,
        "llm_api_key": null,
        "llm_model": null,
        "llm_base_url": null,
        "safety_enabled": null,
        "skills_enabled": null,
        "extensions_enabled": null,
        "max_cost_per_day_cents": 0,
        "config_version": 0,
        "updated_at": "now"
    });

    let client: ClientAdminConfig = serde_json::from_value(server_json).unwrap();
    assert_eq!(client.max_cost_per_day_cents, Some(0));
    assert_eq!(client.config_version, Some(0));
}

#[test]
fn test_contract_report_large_numbers() {
    // 验证大数值在传输中不丢失精度
    let client_report = ClientReport::UsageStats {
        timestamp: "t".into(),
        period_start: "s".into(),
        period_end: "e".into(),
        total_messages: u64::MAX / 2,
        total_tokens: u64::MAX / 2,
        total_cost_cents: u64::MAX / 2,
    };

    let json_str = serde_json::to_string(&client_report).unwrap();
    let server_payload: ServerReportPayload = serde_json::from_str(&json_str).unwrap();

    assert_eq!(
        server_payload.data["total_messages"],
        u64::MAX / 2
    );
}
