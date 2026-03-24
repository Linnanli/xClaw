//! 模型配置 - 单元测试
//!
//! 覆盖 ModelConfig 相关的数据模型序列化、验证逻辑。

use serde_json::json;

/// Admin Backend 的 ModelConfig 相关类型（镜像定义用于测试）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CreateModelConfigRequest {
    model_id: String,
    display_name: String,
    description: Option<String>,
    provider: String,
    api_base_url: Option<String>,
    api_key: Option<String>,
    sort_order: Option<i32>,
    capabilities: Option<serde_json::Value>,
    extra_config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientModelConfig {
    model_id: String,
    display_name: String,
    description: Option<String>,
    provider: String,
    is_default: bool,
    capabilities: serde_json::Value,
}

// ============================================================================
// 正常路径
// ============================================================================

#[test]
fn test_create_request_serialization() {
    let req = CreateModelConfigRequest {
        model_id: "gpt-4o".to_string(),
        display_name: "GPT-4o".to_string(),
        description: Some("最强大的多模态模型".to_string()),
        provider: "openai".to_string(),
        api_base_url: Some("https://api.openai.com/v1".to_string()),
        api_key: Some("sk-test-key".to_string()),
        sort_order: Some(1),
        capabilities: Some(json!(["chat", "vision"])),
        extra_config: None,
    };

    let json_str = serde_json::to_string(&req).unwrap();
    let parsed: CreateModelConfigRequest = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.model_id, "gpt-4o");
    assert_eq!(parsed.display_name, "GPT-4o");
    assert_eq!(parsed.provider, "openai");
}

#[test]
fn test_client_model_config_no_api_key() {
    let config = ClientModelConfig {
        model_id: "gpt-4o".to_string(),
        display_name: "GPT-4o".to_string(),
        description: Some("最强大的多模态模型".to_string()),
        provider: "openai".to_string(),
        is_default: true,
        capabilities: json!(["chat", "vision"]),
    };

    let json_str = serde_json::to_string(&config).unwrap();
    assert!(!json_str.contains("api_key"), "ClientModelConfig 不应包含 api_key");
    assert!(!json_str.contains("api_base_url"), "ClientModelConfig 不应包含 api_base_url");
}

#[test]
fn test_client_model_config_roundtrip() {
    let config = ClientModelConfig {
        model_id: "claude-3.5-sonnet".to_string(),
        display_name: "Claude 3.5 Sonnet".to_string(),
        description: None,
        provider: "anthropic".to_string(),
        is_default: false,
        capabilities: json!(["chat"]),
    };

    let json_val = serde_json::to_value(&config).unwrap();
    let parsed: ClientModelConfig = serde_json::from_value(json_val).unwrap();

    assert_eq!(parsed.model_id, config.model_id);
    assert_eq!(parsed.is_default, false);
    assert!(parsed.description.is_none());
}

// ============================================================================
// 错误路径
// ============================================================================

#[test]
fn test_failure_create_request_missing_required_fields() {
    let json_str = r#"{"model_id": "test"}"#;
    let result: Result<CreateModelConfigRequest, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "缺少 display_name 和 provider 应失败");
}

#[test]
fn test_failure_client_model_config_invalid_json() {
    let result: Result<ClientModelConfig, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

// ============================================================================
// 安全审计
// ============================================================================

#[test]
fn test_audit_client_model_excludes_sensitive_fields() {
    // 模拟从数据库查询后构建 ClientModelConfig 的过程
    // 确保 api_key 和 api_base_url 不会泄露到客户端
    let full_config = json!({
        "model_id": "gpt-4o",
        "display_name": "GPT-4o",
        "description": "Test",
        "provider": "openai",
        "api_base_url": "https://api.openai.com/v1",
        "api_key": "sk-secret-key-12345",
        "is_default": true,
        "capabilities": ["chat"]
    });

    // ClientModelConfig 只取需要的字段
    let client_config = ClientModelConfig {
        model_id: full_config["model_id"].as_str().unwrap().to_string(),
        display_name: full_config["display_name"].as_str().unwrap().to_string(),
        description: full_config["description"].as_str().map(|s| s.to_string()),
        provider: full_config["provider"].as_str().unwrap().to_string(),
        is_default: full_config["is_default"].as_bool().unwrap(),
        capabilities: full_config["capabilities"].clone(),
    };

    let output = serde_json::to_string(&client_config).unwrap();
    assert!(!output.contains("sk-secret"), "客户端配置不应包含 API Key");
    assert!(!output.contains("api.openai.com"), "客户端配置不应包含 API URL");
}
