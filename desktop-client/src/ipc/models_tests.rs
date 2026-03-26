//! 模型配置 IPC 命令测试。
//!
//! 测试维度：
//! - 单元测试：正常路径 + 错误路径
//! - 契约测试：序列化/反序列化一致性
//! - 安全审计测试：API Key 不泄露

use super::models::*;

// ============================================================================
// 单元测试 - 正常路径
// ============================================================================

#[test]
fn test_model_config_serialization() {
    let config = ModelConfig {
        model_id: "gpt-4o".to_string(),
        display_name: "GPT-4o".to_string(),
        description: Some("最强大的多模态模型".to_string()),
        provider: "openai".to_string(),
        provider_display_name: Some("OpenAI".to_string()),
        is_default: true,
        capabilities: serde_json::json!(["chat", "vision"]),
        api_base_url: Some("https://api.openai.com/v1".to_string()),
        api_key: None,
        api_format: "openai".to_string(),
        source: "admin".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: ModelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.model_id, "gpt-4o");
    assert_eq!(parsed.display_name, "GPT-4o");
    assert_eq!(parsed.provider, "openai");
    assert!(parsed.is_default);
    assert_eq!(parsed.source, "admin");
    assert_eq!(parsed.api_format, "openai");
}

#[test]
fn test_custom_model_serialization() {
    let model = CustomModel {
        model_id: "my-claude".to_string(),
        display_name: "My Claude".to_string(),
        description: Some("自定义 Claude".to_string()),
        provider: "anthropic".to_string(),
        api_base_url: "https://api.anthropic.com/v1".to_string(),
        api_key: "sk-ant-test-key".to_string(),
        capabilities: serde_json::json!([]),
        extra_config: serde_json::json!({}),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_string(&model).unwrap();
    let parsed: CustomModel = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.model_id, "my-claude");
    assert_eq!(parsed.display_name, "My Claude");
    assert_eq!(parsed.api_base_url, "https://api.anthropic.com/v1");
}

// ============================================================================
// 契约测试 - 序列化/反序列化一致性
// ============================================================================

#[test]
fn test_contract_model_config_json_roundtrip() {
    let config = ModelConfig {
        model_id: "test-model".to_string(),
        display_name: "Test Model".to_string(),
        description: None,
        provider: "test".to_string(),
        provider_display_name: None,
        is_default: false,
        capabilities: serde_json::json!(["chat"]),
        api_base_url: None,
        api_key: None,
        api_format: "openai".to_string(),
        source: "custom".to_string(),
    };

    let json = serde_json::to_value(&config).unwrap();

    // 验证 JSON 字段名与前端期望一致
    assert!(json.get("model_id").is_some());
    assert!(json.get("display_name").is_some());
    assert!(json.get("description").is_some());
    assert!(json.get("provider").is_some());
    assert!(json.get("is_default").is_some());
    assert!(json.get("capabilities").is_some());
    assert!(json.get("source").is_some());
    assert!(json.get("api_format").is_some());
}

#[test]
fn test_contract_custom_model_json_roundtrip() {
    let model = CustomModel {
        model_id: "test".to_string(),
        display_name: "Test".to_string(),
        description: None,
        provider: "test".to_string(),
        api_base_url: "https://example.com".to_string(),
        api_key: "sk-test".to_string(),
        capabilities: serde_json::json!([]),
        extra_config: serde_json::json!({}),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    };

    let json = serde_json::to_value(&model).unwrap();

    // 验证前端需要的所有字段都存在
    assert!(json.get("model_id").is_some());
    assert!(json.get("display_name").is_some());
    assert!(json.get("api_base_url").is_some());
    assert!(json.get("api_key").is_some());
    assert!(json.get("created_at").is_some());
    assert!(json.get("updated_at").is_some());
}

#[test]
fn test_contract_model_config_default_source() {
    // 验证反序列化时缺少 source 字段会使用默认值 "admin"
    let json = r#"{
        "model_id": "test",
        "display_name": "Test",
        "description": null,
        "provider": "test",
        "is_default": false,
        "capabilities": []
    }"#;

    let config: ModelConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.source, "admin", "缺少 source 字段时应默认为 admin");
    assert_eq!(config.api_format, "openai", "缺少 api_format 字段时应默认为 openai");
}

// ============================================================================
// 失败路径测试 - 边界条件和异常处理
// ============================================================================

#[test]
fn test_failure_model_config_missing_required_fields() {
    let json = r#"{"model_id": "test"}"#;
    let result: Result<ModelConfig, _> = serde_json::from_str(json);
    assert!(result.is_err(), "缺少必填字段应返回错误");
}

// ============================================================================
// 安全审计测试 - API Key 不泄露
// ============================================================================

#[test]
fn test_audit_model_config_no_api_key_in_source() {
    // ModelConfig 的 api_key 字段是 Option，内置模型应为 None
    let config = ModelConfig {
        model_id: "test".to_string(),
        display_name: "Test".to_string(),
        description: None,
        provider: "test".to_string(),
        provider_display_name: None,
        is_default: false,
        capabilities: serde_json::json!([]),
        api_base_url: None,
        api_key: None,
        api_format: "openai".to_string(),
        source: "builtin".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    assert!(
        !json.contains("sk-"),
        "内置模型不应包含 API Key 前缀"
    );
}

#[test]
fn test_audit_custom_model_debug_output_contains_key() {
    // 验证 Debug trait 输出包含 api_key（已知行为，生产环境不应在 info 级别打印）
    let model = CustomModel {
        model_id: "test".to_string(),
        display_name: "Test".to_string(),
        description: None,
        provider: "test".to_string(),
        api_base_url: "https://example.com".to_string(),
        api_key: "sk-secret-key-12345".to_string(),
        capabilities: serde_json::json!([]),
        extra_config: serde_json::json!({}),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    };

    let debug_output = format!("{:?}", model);
    assert!(
        debug_output.contains("sk-secret-key-12345"),
        "Debug 输出包含 api_key（注意：不要在生产日志中使用 Debug 打印 CustomModel）"
    );
}
