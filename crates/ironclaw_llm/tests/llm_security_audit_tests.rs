//! ironclaw_llm — 安全审计测试
//!
//! 验证 API Key 不泄露、敏感信息不出现在日志/错误信息中。

use ironclaw_llm::{
    LlmError, RegistryProviderConfig, ProviderProtocol,
    config::CacheRetention,
    create_provider_from_config,
};
use secrecy::{ExposeSecret, SecretString};

// ============================================================================
// API Key 安全
// ============================================================================

#[test]
fn test_audit_secret_string_not_in_debug() {
    let secret = SecretString::from("sk-super-secret-key-12345");
    let debug_output = format!("{:?}", secret);
    assert!(!debug_output.contains("sk-super-secret"), "SecretString 的 Debug 不应暴露密钥");
}

#[test]
fn test_audit_secret_string_expose_explicit() {
    let secret = SecretString::from("sk-test-key");
    // 只有显式调用 expose_secret() 才能获取值
    assert_eq!(secret.expose_secret(), "sk-test-key");
}

#[test]
fn test_audit_registry_config_debug_no_key() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "deepseek".to_string(),
        api_key: Some(SecretString::from("sk-real-api-key-do-not-leak")),
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

    let debug = format!("{:?}", config);
    assert!(!debug.contains("sk-real-api-key"), "Debug 输出不应包含 API Key 明文");
}

// ============================================================================
// 错误信息安全
// ============================================================================

#[test]
fn test_audit_auth_error_no_key_in_message() {
    let error = LlmError::AuthFailed {
        provider: "deepseek".to_string(),
    };
    let msg = format!("{}", error);
    assert!(!msg.contains("sk-"), "错误信息不应包含 API Key");
}

#[test]
fn test_audit_request_error_no_sensitive_data() {
    let error = LlmError::RequestFailed {
        provider: "openai".to_string(),
        reason: "HTTP 401 Unauthorized".to_string(),
    };
    let msg = format!("{}", error);
    assert!(!msg.contains("sk-"), "错误信息不应包含 API Key");
    assert!(!msg.contains("Bearer"), "错误信息不应包含 Authorization header");
}

// ============================================================================
// Provider 创建安全
// ============================================================================

#[test]
fn test_audit_provider_model_name_safe() {
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "deepseek".to_string(),
        api_key: Some(SecretString::from("sk-secret-key")),
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

    let provider = create_provider_from_config(&config).unwrap();
    let model_name = provider.model_name();
    assert!(!model_name.contains("sk-"), "model_name 不应包含 API Key");
    assert_eq!(model_name, "deepseek-chat");
}

// ============================================================================
// 截断函数安全
// ============================================================================

#[test]
fn test_audit_truncate_does_not_leak_full_content() {
    let sensitive = "用户身份证号: 330326199408015618，银行卡号: 6222021234567890";
    let truncated = ironclaw_llm::util::truncate_for_preview(sensitive, 20);
    // 截断后不应包含完整的敏感信息
    assert!(!truncated.contains("6222021234567890"), "截断后不应包含完整银行卡号");
}
