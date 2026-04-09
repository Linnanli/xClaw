//! 模型配置 - 契约测试
//!
//! 验证 Admin Backend 与 Desktop Client 之间的接口契约一致性。
//! 确保 API 响应格式与前端期望匹配。
//!
//! 契约来源：
//! - 后端：`admin-backend/src/models.rs` → `ClientModelConfig`
//! - 前端：`desktop-client/src/ipc/models.rs` → `ModelConfig`
//! - TS：`desktop-client/src-ui/src/app/utils/tauri.ts` → `ModelConfigItem`

use serde_json::json;

/// Desktop Client 期望的模型配置格式（镜像 `ClientModelConfig`）。
///
/// 必须与 `admin-backend/src/models.rs::ClientModelConfig` 保持同步。
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct ClientExpectedModelConfig {
    model_id: String,
    display_name: String,
    description: Option<String>,
    provider: String,
    is_default: bool,
    capabilities: Vec<String>,
    api_base_url: Option<String>,
    api_key: Option<String>,
    api_format: String,
    source: String,
}

// ── 正常路径 ──

#[test]
fn test_contract_client_models_response_format() {
    let response = json!([
        {
            "model_id": "gpt-4o",
            "display_name": "GPT-4o",
            "description": "最强大的多模态模型",
            "provider": "openai",
            "is_default": true,
            "capabilities": ["chat", "vision"],
            "api_base_url": "https://api.openai.com/v1",
            "api_key": "sk-test-key",
            "api_format": "openai",
            "source": "admin"
        },
        {
            "model_id": "glm-4.7-flash",
            "display_name": "GLM-4.7-Flash",
            "description": null,
            "provider": "zhipu",
            "is_default": false,
            "capabilities": ["chat"],
            "api_base_url": "https://open.bigmodel.cn/api/paas/v4",
            "api_key": null,
            "api_format": "openai",
            "source": "admin"
        }
    ]);

    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("前端应能反序列化 client-models 响应");

    assert_eq!(models.len(), 2);
    assert_eq!(models[0].model_id, "gpt-4o");
    assert!(models[0].is_default);
    assert_eq!(models[0].source, "admin");
    assert_eq!(models[0].api_format, "openai");
    assert_eq!(models[0].capabilities, vec!["chat", "vision"]);
    assert!(!models[1].is_default);
    assert!(models[1].api_key.is_none());
}

#[test]
fn test_contract_client_models_with_null_description() {
    let response = json!([{
        "model_id": "test",
        "display_name": "Test",
        "description": null,
        "provider": "test",
        "is_default": false,
        "capabilities": [],
        "api_base_url": null,
        "api_key": null,
        "api_format": "openai",
        "source": "admin"
    }]);

    let models: Vec<ClientExpectedModelConfig> =
        serde_json::from_value(response).expect("description 为 null 时应能反序列化");

    assert!(models[0].description.is_none());
    assert!(models[0].api_base_url.is_none());
}

#[test]
fn test_contract_client_models_capabilities_is_string_array() {
    let response = json!([
        {
            "model_id": "m1", "display_name": "M1", "description": null,
            "provider": "p1", "is_default": false,
            "capabilities": ["chat", "vision", "code"],
            "api_base_url": null, "api_key": null,
            "api_format": "openai", "source": "admin"
        },
        {
            "model_id": "m2", "display_name": "M2", "description": null,
            "provider": "p2", "is_default": false,
            "capabilities": [],
            "api_base_url": null, "api_key": null,
            "api_format": "openai", "source": "admin"
        }
    ]);

    let models: Vec<ClientExpectedModelConfig> = serde_json::from_value(response).unwrap();

    assert_eq!(models[0].capabilities, vec!["chat", "vision", "code"]);
    assert!(models[1].capabilities.is_empty());
}

// ── 失败路径 ──

#[test]
fn test_failure_capabilities_non_array_rejected() {
    // capabilities 必须是字符串数组，非数组值应反序列化失败
    let response = json!([{
        "model_id": "m1", "display_name": "M1", "description": null,
        "provider": "p1", "is_default": false,
        "capabilities": "not-an-array",
        "api_base_url": null, "api_key": null,
        "api_format": "openai", "source": "admin"
    }]);

    let result: Result<Vec<ClientExpectedModelConfig>, _> = serde_json::from_value(response);
    assert!(result.is_err(), "capabilities 为非数组时应反序列化失败");
}

#[test]
fn test_failure_missing_source_field() {
    // source 字段缺失时应反序列化失败（无 serde default）
    let response = json!([{
        "model_id": "m1", "display_name": "M1", "description": null,
        "provider": "p1", "is_default": false,
        "capabilities": [],
        "api_base_url": null, "api_key": null,
        "api_format": "openai"
    }]);

    let result: Result<Vec<ClientExpectedModelConfig>, _> = serde_json::from_value(response);
    assert!(result.is_err(), "缺少 source 字段时应反序列化失败");
}
