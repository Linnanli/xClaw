//! 模型配置 IPC 命令测试。
//!
//! 测试维度：
//! - 单元测试：正常路径 + 错误路径
//! - 契约测试：序列化/反序列化一致性
//! - 安全审计测试：API Key 不泄露

use super::models::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    assert_eq!(
        config.api_format, "openai",
        "缺少 api_format 字段时应默认为 openai"
    );
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

#[test]
fn test_client_models_url_includes_client_id() {
    let client_id =
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440999").expect("uuid should parse");

    let url = client_models_url("https://admin.example.com", client_id);

    assert_eq!(
        url,
        "https://admin.example.com/api/client-models?client_id=550e8400-e29b-41d4-a716-446655440999"
    );
}

#[test]
#[serial_test::serial]
fn test_model_fetch_client_id_requires_token() {
    let _admin_token = EnvVarGuard::unset("ADMIN_AUTH_TOKEN");

    let error = model_fetch_client_id().expect_err("missing client token must fail model fetch");

    assert!(error.contains("ADMIN_AUTH_TOKEN 未配置"));
}

#[test]
#[serial_test::serial]
fn test_model_fetch_client_id_rejects_non_uuid_token() {
    let _admin_token = EnvVarGuard::set("ADMIN_AUTH_TOKEN", "not-a-uuid".to_string());

    let error = model_fetch_client_id().expect_err("invalid client token must fail model fetch");

    assert!(error.contains("不是有效客户端 ID"));
}

#[tokio::test]
#[serial_test::serial]
async fn test_contract_admin_models_success_is_authoritative_even_when_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/client-models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let _admin_url = AdminBackendUrlGuard::set(server.uri());
    let models = fetch_admin_models(uuid::Uuid::nil()).await;

    assert!(
        models
            .expect("successful empty Admin response should parse")
            .is_empty(),
        "空 Admin 响应不能降级到 provider active model"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_failure_admin_models_error_is_not_masked_as_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/client-models"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let _admin_url = AdminBackendUrlGuard::set(server.uri());
    let error = fetch_admin_models(uuid::Uuid::nil())
        .await
        .expect_err("Admin failure should remain visible to IPC");

    assert!(
        error.contains("HTTP 503"),
        "Admin 失败时应返回具体错误，不能静默变成空列表"
    );
}

struct AdminBackendUrlGuard {
    previous_url: Option<String>,
}

impl AdminBackendUrlGuard {
    fn set(url: String) -> Self {
        let previous_url = std::env::var("ADMIN_BACKEND_URL").ok();
        std::env::set_var("ADMIN_BACKEND_URL", url);
        Self { previous_url }
    }
}

impl Drop for AdminBackendUrlGuard {
    fn drop(&mut self) {
        match &self.previous_url {
            Some(url) => std::env::set_var("ADMIN_BACKEND_URL", url),
            None => std::env::remove_var("ADMIN_BACKEND_URL"),
        }
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous_value: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: String) -> Self {
        let previous_value = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key,
            previous_value,
        }
    }

    fn unset(key: &'static str) -> Self {
        let previous_value = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key,
            previous_value,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous_value {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
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
    assert!(!json.contains("sk-"), "内置模型不应包含 API Key 前缀");
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

#[tokio::test]
async fn req_models_i5_invalid_model_connection_returns_recoverable_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_json(serde_json::json!({
            "model": "invalid-model",
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 5,
        })))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": {"message": "model not found"}
        })))
        .mount(&server)
        .await;

    let result = test_model_connection(
        server.uri(),
        "test-key".to_string(),
        "invalid-model".to_string(),
    )
    .await
    .expect("HTTP error status should be returned as recoverable payload");

    assert_eq!(result["success"], false);
    assert_eq!(result["status"], 404);
    assert!(result["message"]
        .as_str()
        .unwrap_or_default()
        .contains("连接失败"));
    assert!(result["error"]
        .as_str()
        .unwrap_or_default()
        .contains("model not found"));
}

#[tokio::test]
async fn req_models_i5_valid_model_connection_reports_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{"message": {"content": "ok"}}]
        })))
        .mount(&server)
        .await;

    let result = test_model_connection(
        server.uri(),
        "test-key".to_string(),
        "valid-model".to_string(),
    )
    .await
    .expect("successful probe should return payload");

    assert_eq!(result["success"], true);
    assert_eq!(result["status"], 200);
}
