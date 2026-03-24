//! 模型配置 - 契约测试
//!
//! 验证 Admin Backend 与 Desktop Client 之间的接口契约一致性。
//! 确保 API 响应格式与前端期望匹配。

use serde_json::json;

/// Desktop Client 期望的模型配置格式
#[derive(Debug, serde::Deserialize)]
struct ClientExpectedModelConfig {
    model_id: String,
    display_name: String,
    description: Option<String>,
    provider: String,
    is_default: bool,
    capabilities: serde_json::Value,
}

// ============================================================================
// 契约测试
// ============================================================================

#[test]
fn test_contract_client_models_response_format() {
    // 模拟 GET /api/client-models 的响应
    let response = json!([
        {
            "model_id": "gpt-4o",
            "display_name": "GPT-4o",
            "description": "最强大的多模态模型",
            "provider": "openai",
            "is_default": true,
            "capabilities": ["chat", "vision"]
        },
        {
            "model_id": "gpt-4o-mini",
            "display_name": "GPT-4o Mini",
            "description": "快速且经济",
            "provider": "openai",
            "is_default": false,
            "capabilities": ["chat"]
        }
    ]);

    // 验证前端可以正确反序列化
    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("前端应能反序列化 client-models 响应");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].model_id, "gpt-4o");
    assert!(models[0].is_default);
    assert!(!models[1].is_default);
}

#[test]
fn test_contract_client_models_with_null_description() {
    let response = json!([{
        "model_id": "test",
        "display_name": "Test",
        "description": null,
        "provider": "test",
        "is_default": false,
        "capabilities": []
    }]);

    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("description 为 null 时应能反序列化");

    assert!(models[0].description.is_none());
}

#[test]
fn test_contract_client_models_capabilities_types() {
    // capabilities 可以是字符串数组或空数组
    let response = json!([
        {
            "model_id": "m1",
            "display_name": "M1",
            "description": null,
            "provider": "p1",
            "is_default": false,
            "capabilities": ["chat", "vision", "code"]
        },
        {
            "model_id": "m2",
            "display_name": "M2",
            "description": null,
            "provider": "p2",
            "is_default": false,
            "capabilities": []
        }
    ]);

    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).unwrap();

    assert!(models[0].capabilities.is_array());
    assert_eq!(models[0].capabilities.as_array().unwrap().len(), 3);
    assert!(models[1].capabilities.as_array().unwrap().is_empty());
}

#[test]
fn test_contract_model_config_admin_response_format() {
    // 模拟 GET /api/model-configs 的管理端响应（包含更多字段）
    let response = json!([{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "model_id": "gpt-4o",
        "display_name": "GPT-4o",
        "description": "最强大的多模态模型",
        "provider": "openai",
        "api_base_url": "https://api.openai.com/v1",
        "api_key": "sk-***",
        "enabled": true,
        "is_default": true,
        "sort_order": 1,
        "capabilities": ["chat", "vision"],
        "extra_config": {},
        "created_at": "2025-01-01T00:00:00Z",
        "updated_at": "2025-01-01T00:00:00Z"
    }]);

    // 管理端响应应包含所有字段
    let configs: Vec<serde_json::Value> = serde_json::from_value(response).unwrap();
    let config = &configs[0];

    assert!(config.get("id").is_some(), "管理端响应应包含 id");
    assert!(config.get("api_base_url").is_some(), "管理端响应应包含 api_base_url");
    assert!(config.get("api_key").is_some(), "管理端响应应包含 api_key");
    assert!(config.get("enabled").is_some(), "管理端响应应包含 enabled");
    assert!(config.get("sort_order").is_some(), "管理端响应应包含 sort_order");
}
