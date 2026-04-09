//! 模型配置 - 单元测试
//!
//! 覆盖 ModelConfig 相关的数据模型序列化、验证逻辑、URL 规范化。

use serde_json::json;

/// Admin Backend 的创建请求（镜像定义用于测试）
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

/// 客户端模型配置（镜像 `ClientModelConfig`，含完整字段）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ClientModelConfig {
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
fn test_client_model_config_roundtrip() {
    let config = ClientModelConfig {
        model_id: "claude-3.5-sonnet".to_string(),
        display_name: "Claude 3.5 Sonnet".to_string(),
        description: None,
        provider: "anthropic".to_string(),
        is_default: false,
        capabilities: vec!["chat".to_string()],
        api_base_url: Some("https://api.anthropic.com".to_string()),
        api_key: Some("sk-ant-test".to_string()),
        api_format: "anthropic".to_string(),
        source: "admin".to_string(),
    };

    let json_val = serde_json::to_value(&config).unwrap();
    let parsed: ClientModelConfig = serde_json::from_value(json_val).unwrap();

    assert_eq!(parsed.model_id, config.model_id);
    assert!(!parsed.is_default);
    assert!(parsed.description.is_none());
    assert_eq!(parsed.source, "admin");
    assert_eq!(parsed.capabilities, vec!["chat"]);
}

#[test]
fn test_client_model_config_with_all_capabilities() {
    let config = ClientModelConfig {
        model_id: "gpt-4o".to_string(),
        display_name: "GPT-4o".to_string(),
        description: Some("多模态".to_string()),
        provider: "openai".to_string(),
        is_default: true,
        capabilities: vec!["chat".to_string(), "vision".to_string(), "code".to_string()],
        api_base_url: Some("https://api.openai.com/v1".to_string()),
        api_key: None,
        api_format: "openai".to_string(),
        source: "admin".to_string(),
    };

    let json_val = serde_json::to_value(&config).unwrap();
    let caps = json_val["capabilities"].as_array().unwrap();
    assert_eq!(caps.len(), 3);
    assert_eq!(caps[0].as_str().unwrap(), "chat");
}

// ── URL 规范化 ──

/// 镜像 `admin-backend/src/routes.rs::normalize_api_base_url`
fn normalize_api_base_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    for suffix in &["/chat/completions", "/completions", "/messages"] {
        if let Some(base) = trimmed.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    trimmed.to_string()
}

#[test]
fn test_normalize_strips_chat_completions() {
    // /v1 是 base URL 的一部分，保留
    assert_eq!(
        normalize_api_base_url("https://api.openai.com/v1/chat/completions"),
        "https://api.openai.com/v1"
    );
}

#[test]
fn test_normalize_strips_bare_chat_completions() {
    assert_eq!(
        normalize_api_base_url("https://open.bigmodel.cn/api/paas/v4/chat/completions"),
        "https://open.bigmodel.cn/api/paas/v4"
    );
}

#[test]
fn test_normalize_strips_anthropic_messages() {
    // /v1 保留，只剥离 /messages
    assert_eq!(
        normalize_api_base_url("https://api.anthropic.com/v1/messages"),
        "https://api.anthropic.com/v1"
    );
}

#[test]
fn test_normalize_strips_bare_messages() {
    assert_eq!(
        normalize_api_base_url("https://api.example.com/messages"),
        "https://api.example.com"
    );
}

#[test]
fn test_normalize_preserves_v1_suffix() {
    // /v1 是 base URL 的一部分，admin 端不应剥离
    assert_eq!(
        normalize_api_base_url("https://api.openai.com/v1"),
        "https://api.openai.com/v1"
    );
}

#[test]
fn test_normalize_strips_trailing_slash() {
    assert_eq!(
        normalize_api_base_url("https://api.openai.com/v1/"),
        "https://api.openai.com/v1"
    );
}

#[test]
fn test_normalize_clean_url_unchanged() {
    assert_eq!(
        normalize_api_base_url("https://api.deepseek.com"),
        "https://api.deepseek.com"
    );
}

#[test]
fn test_normalize_strips_bare_completions() {
    assert_eq!(
        normalize_api_base_url("https://api.example.com/completions"),
        "https://api.example.com"
    );
}

// ── 错误路径 ──

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
