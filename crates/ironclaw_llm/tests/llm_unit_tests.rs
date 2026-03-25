//! ironclaw_llm — 单元测试
//!
//! 覆盖核心类型、消息构造、成本计算、provider registry 等。

use ironclaw_llm::{
    ChatMessage, CompletionRequest, Role, ToolDefinition, ToolCall,
    RegistryProviderConfig, LlmError, CacheRetention,
    ProviderProtocol, ProviderRegistry,
};
use ironclaw_llm::costs;
use ironclaw_llm::util::truncate_for_preview;

// ============================================================================
// 消息构造
// ============================================================================

#[test]
fn req_llm_001_user_message_construction() {
    let msg = ChatMessage::user("你好");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.content, "你好");
}

#[test]
fn req_llm_002_assistant_message_construction() {
    let msg = ChatMessage::assistant("你好！有什么可以帮你的？");
    assert_eq!(msg.role, Role::Assistant);
    assert!(!msg.content.is_empty());
}

#[test]
fn req_llm_003_system_message_construction() {
    let msg = ChatMessage::system("你是一个有用的助手");
    assert_eq!(msg.role, Role::System);
}

#[test]
fn req_llm_004_completion_request_builder() {
    let messages = vec![
        ChatMessage::system("你是助手"),
        ChatMessage::user("你好"),
    ];
    let request = CompletionRequest::new(messages)
        .with_model("deepseek-chat")
        .with_max_tokens(1000)
        .with_temperature(0.7);

    assert_eq!(request.model.as_deref(), Some("deepseek-chat"));
    assert_eq!(request.max_tokens, Some(1000));
}

// ============================================================================
// Provider Registry
// ============================================================================

#[test]
fn req_llm_005_registry_loads_builtin_providers() {
    let registry = ProviderRegistry::load();
    // 至少应该有 openai、anthropic、ollama、deepseek
    assert!(registry.find("openai").is_some(), "应包含 openai");
    assert!(registry.find("anthropic").is_some(), "应包含 anthropic");
    assert!(registry.find("ollama").is_some(), "应包含 ollama");
}

#[test]
fn req_llm_006_registry_find_deepseek() {
    let registry = ProviderRegistry::load();
    let def = registry.find("deepseek");
    assert!(def.is_some(), "应包含 deepseek");
}

#[test]
fn req_llm_007_registry_protocol_mapping() {
    let registry = ProviderRegistry::load();
    if let Some(def) = registry.find("openai") {
        assert!(matches!(def.protocol, ProviderProtocol::OpenAiCompletions));
    }
    if let Some(def) = registry.find("anthropic") {
        assert!(matches!(def.protocol, ProviderProtocol::Anthropic));
    }
    if let Some(def) = registry.find("ollama") {
        assert!(matches!(def.protocol, ProviderProtocol::Ollama));
    }
}

// ============================================================================
// 工具函数
// ============================================================================

#[test]
fn req_llm_008_truncate_for_preview_short() {
    let result = truncate_for_preview("hello", 10);
    assert_eq!(result, "hello");
}

#[test]
fn req_llm_009_truncate_for_preview_long() {
    let result = truncate_for_preview("hello world this is a long string", 10);
    assert_eq!(result.len(), 10 + "…".len());
    assert!(result.ends_with('…'));
}

// ============================================================================
// CacheRetention
// ============================================================================

#[test]
fn req_llm_010_cache_retention_default() {
    let retention = CacheRetention::default();
    assert_eq!(retention, CacheRetention::Short);
}

#[test]
fn req_llm_011_cache_retention_parse() {
    assert_eq!("none".parse::<CacheRetention>().unwrap(), CacheRetention::None);
    assert_eq!("short".parse::<CacheRetention>().unwrap(), CacheRetention::Short);
    assert_eq!("long".parse::<CacheRetention>().unwrap(), CacheRetention::Long);
}
