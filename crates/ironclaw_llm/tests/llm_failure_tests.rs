//! ironclaw_llm — 失败路径测试
//!
//! 覆盖 provider 创建失败、配置缺失、无效参数等场景。

use ironclaw_llm::{
    ChatMessage, CompletionRequest, LlmError,
    RegistryProviderConfig, ProviderProtocol,
    config::CacheRetention,
    create_provider_from_config,
};
use secrecy::SecretString;

// ============================================================================
// Provider 创建失败
// ============================================================================

#[test]
fn test_failure_anthropic_no_api_key() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::Anthropic,
        provider_id: "anthropic".to_string(),
        api_key: None, // 缺少 API Key
        base_url: "https://api.anthropic.com".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    let result = create_provider_from_config(&config);
    assert!(result.is_err(), "Anthropic 缺少 API Key 应失败");
}

#[test]
fn test_failure_openai_no_key_still_creates_provider() {
    // OpenAI 兼容模式：没有 API Key 也能创建 provider（会在请求时失败）
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "deepseek".to_string(),
        api_key: None,
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

    // OpenAI 兼容模式允许无 key 创建（用 "no-key" 占位）
    let result = create_provider_from_config(&config);
    assert!(result.is_ok(), "OpenAI 兼容模式应允许无 key 创建");
}

// ============================================================================
// 消息构造边界条件
// ============================================================================

#[test]
fn test_failure_empty_message_content() {
    let msg = ChatMessage::user("");
    assert_eq!(msg.content, "");
    // 空消息在构造时不报错，由 LLM API 返回错误
}

#[test]
fn test_failure_very_long_message() {
    let long_content = "a".repeat(100_000);
    let msg = ChatMessage::user(&long_content);
    assert_eq!(msg.content.len(), 100_000);
}

#[test]
fn test_failure_empty_completion_request() {
    let request = CompletionRequest::new(vec![]);
    assert!(request.messages.is_empty());
    // 空消息列表在构造时不报错，由 LLM API 返回错误
}

// ============================================================================
// LlmError 类型
// ============================================================================

#[test]
fn test_failure_error_display() {
    let error = LlmError::AuthFailed {
        provider: "deepseek".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("deepseek") || display.contains("auth"), "错误信息应包含 provider 名称");
}

#[test]
fn test_failure_request_failed_error() {
    let error = LlmError::RequestFailed {
        provider: "openai".to_string(),
        reason: "Connection timeout".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("timeout") || display.contains("openai"));
}

// ============================================================================
// Registry 失败路径
// ============================================================================

#[test]
fn test_failure_registry_unknown_provider() {
    let registry = ironclaw_llm::ProviderRegistry::load();
    let result = registry.find("nonexistent_provider_xyz");
    assert!(result.is_none(), "未知 provider 应返回 None");
}

#[test]
fn test_failure_cache_retention_invalid_parse() {
    let result = "invalid_value".parse::<CacheRetention>();
    assert!(result.is_err(), "无效的 cache retention 值应解析失败");
}
