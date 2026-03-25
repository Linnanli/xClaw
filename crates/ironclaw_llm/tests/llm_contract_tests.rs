//! ironclaw_llm — 契约测试
//!
//! 验证 LlmProvider trait 的接口契约、RegistryProviderConfig 的序列化格式、
//! 以及 create_provider_from_config 的行为。

use ironclaw_llm::{
    ChatMessage, CompletionRequest, CompletionResponse, FinishReason,
    LlmProvider, RegistryProviderConfig, ProviderProtocol,
    ToolDefinition, ToolCall, ToolCompletionRequest,
    config::CacheRetention,
    create_provider_from_config,
};
use secrecy::SecretString;

// ============================================================================
// LlmProvider trait 契约
// ============================================================================

#[test]
fn test_contract_completion_response_has_required_fields() {
    // CompletionResponse 必须包含 content、input_tokens、output_tokens、finish_reason
    let response = CompletionResponse {
        content: "你好！".to_string(),
        input_tokens: 10,
        output_tokens: 5,
        finish_reason: FinishReason::Stop,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
    };

    assert!(!response.content.is_empty());
    assert!(response.input_tokens > 0);
    assert!(response.output_tokens > 0);
    assert!(matches!(response.finish_reason, FinishReason::Stop));
}

#[test]
fn test_contract_finish_reason_variants() {
    // FinishReason 必须支持 Stop 和 Length
    let _stop = FinishReason::Stop;
    let _length = FinishReason::Length;
}

#[test]
fn test_contract_tool_definition_format() {
    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "获取天气信息".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            }
        }),
    };

    assert_eq!(tool.name, "get_weather");
    assert!(!tool.description.is_empty());
    assert!(tool.parameters.is_object());
}

#[test]
fn test_contract_tool_call_format() {
    let call = ToolCall {
        id: "call_123".to_string(),
        name: "get_weather".to_string(),
        arguments: serde_json::json!({"location": "北京"}),
    };

    assert!(!call.id.is_empty());
    assert_eq!(call.name, "get_weather");
}

// ============================================================================
// RegistryProviderConfig 契约
// ============================================================================

#[test]
fn test_contract_registry_config_openai_compat() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "deepseek".to_string(),
        api_key: Some(SecretString::from("sk-test")),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    assert!(matches!(config.protocol, ProviderProtocol::OpenAiCompletions));
    assert_eq!(config.provider_id, "deepseek");
    assert!(!config.base_url.is_empty());
}

#[test]
fn test_contract_registry_config_anthropic() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::Anthropic,
        provider_id: "anthropic".to_string(),
        api_key: Some(SecretString::from("sk-ant-test")),
        base_url: "https://api.anthropic.com".to_string(),
        model: "claude-3-5-sonnet-20241022".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::Short,
        unsupported_params: vec![],
    };

    assert!(matches!(config.protocol, ProviderProtocol::Anthropic));
}

#[test]
fn test_contract_registry_config_ollama() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::Ollama,
        provider_id: "ollama".to_string(),
        api_key: None,
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    assert!(matches!(config.protocol, ProviderProtocol::Ollama));
    assert!(config.api_key.is_none(), "Ollama 不需要 API Key");
}

// ============================================================================
// create_provider_from_config 契约
// ============================================================================

#[test]
fn test_contract_create_provider_openai_compat() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "deepseek".to_string(),
        api_key: Some(SecretString::from("sk-test")),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    let provider = create_provider_from_config(&config);
    assert!(provider.is_ok(), "OpenAI 兼容 provider 创建应成功");
    assert_eq!(provider.unwrap().model_name(), "deepseek-chat");
}

#[test]
fn test_contract_create_provider_ollama() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::Ollama,
        provider_id: "ollama".to_string(),
        api_key: None,
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    let provider = create_provider_from_config(&config);
    assert!(provider.is_ok(), "Ollama provider 创建应成功");
}
