//! 模型配置 IPC 命令测试。
//!
//! 测试维度：
//! - 单元测试：正常路径 + 错误路径
//! - 契约测试：序列化/反序列化一致性
//! - 失败路径测试：边界条件和异常处理
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
        is_default: true,
        capabilities: serde_json::json!(["chat", "vision"]),
        source: "admin".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: ModelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.model_id, "gpt-4o");
    assert_eq!(parsed.display_name, "GPT-4o");
    assert_eq!(parsed.provider, "openai");
    assert!(parsed.is_default);
    assert_eq!(parsed.source, "admin");
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

#[test]
fn test_builtin_models_not_empty() {
    let models = builtin_models();
    assert!(!models.is_empty(), "内置模型列表不应为空");
    assert!(
        models.iter().any(|m| m.is_default),
        "应有至少一个默认模型"
    );
}

#[test]
fn test_builtin_models_have_valid_fields() {
    for model in builtin_models() {
        assert!(!model.model_id.is_empty(), "model_id 不应为空");
        assert!(!model.display_name.is_empty(), "display_name 不应为空");
        assert!(!model.provider.is_empty(), "provider 不应为空");
        assert_eq!(model.source, "builtin");
    }
}

#[test]
fn test_default_source_is_admin() {
    assert_eq!(default_source(), "admin");
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
        is_default: false,
        capabilities: serde_json::json!(["chat"]),
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
}

// ============================================================================
// 失败路径测试 - 边界条件和异常处理
// ============================================================================

#[test]
fn test_failure_custom_model_store_empty_deserialization() {
    let json = r#"{"models": []}"#;
    let store: CustomModelStore = serde_json::from_str(json).unwrap();
    assert!(store.models.is_empty());
}

#[test]
fn test_failure_custom_model_store_invalid_json() {
    let result: Result<CustomModelStore, _> = serde_json::from_str("invalid json");
    assert!(result.is_err(), "无效 JSON 应返回错误");
}

#[test]
fn test_failure_model_config_missing_required_fields() {
    let json = r#"{"model_id": "test"}"#;
    let result: Result<ModelConfig, _> = serde_json::from_str(json);
    assert!(result.is_err(), "缺少必填字段应返回错误");
}

#[test]
fn test_failure_custom_model_store_default() {
    let store = CustomModelStore::default();
    assert!(store.models.is_empty(), "默认 store 应为空");
}

// ============================================================================
// 安全审计测试 - API Key 不泄露
// ============================================================================

#[test]
fn test_audit_model_config_no_api_key_exposure() {
    let config = ModelConfig {
        model_id: "test".to_string(),
        display_name: "Test".to_string(),
        description: None,
        provider: "test".to_string(),
        is_default: false,
        capabilities: serde_json::json!([]),
        source: "admin".to_string(),
    };

    let json = serde_json::to_string(&config).unwrap();
    // ModelConfig 不应包含 api_key 字段（仅 CustomModel 有）
    assert!(
        !json.contains("api_key"),
        "ModelConfig 序列化不应包含 api_key"
    );
}

#[test]
fn test_audit_builtin_models_no_sensitive_data() {
    for model in builtin_models() {
        let json = serde_json::to_string(&model).unwrap();
        assert!(
            !json.contains("sk-"),
            "内置模型不应包含 API Key 前缀"
        );
        assert!(
            !json.contains("api_key"),
            "内置模型不应包含 api_key 字段"
        );
    }
}

#[test]
fn test_audit_custom_model_debug_output_contains_key() {
    // 验证 Debug trait 输出包含 api_key（这是预期行为，但需要注意日志级别）
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
    // 注意：Debug 输出会包含 api_key，生产环境不应在 info/warn 级别打印
    // 这个测试记录了这个已知行为
    assert!(
        debug_output.contains("sk-secret-key-12345"),
        "Debug 输出包含 api_key（注意：不要在生产日志中使用 Debug 打印 CustomModel）"
    );
}
