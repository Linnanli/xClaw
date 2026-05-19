//! LLM integration — provider-level building blocks.
//!
//! Per ADR-118 / ADR-129 (F3.1 verbatim port), this module holds only the
//! provider-pure pieces of what was historically `ironclaw/src/llm/`:
//! HTTP clients, OAuth flows, retry / failover / cache decorators, prompt
//! assembly, model metadata, and registry.
//!
//! Application-coupled modules — session lifecycle, recording, transcription,
//! NEAR AI chat client (which embeds session state), and the `create_*`
//! factory tree — live in `desktop-client/ironclaw/src/llm/`. They are
//! re-exported via `ironclaw::llm` so call-sites keep working.
//!
//! Supported backends:
//! - **NEAR AI**: Session token or API key auth via Chat Completions API
//! - **OpenAI**: Direct API access with your own key
//! - **Anthropic**: Direct API access with your own key
//! - **Ollama**: Local model inference
//! - **OpenAI-compatible**: Any endpoint that speaks the OpenAI API
//! - **AWS Bedrock**: Native Converse API via aws-sdk-bedrockruntime

#[cfg(feature = "bedrock")]
pub mod bedrock;
pub mod circuit_breaker;
pub mod claw_code_provider;
pub mod codex_auth;
pub mod codex_chatgpt;
pub mod config;
pub mod costs;
pub mod error;
pub mod failover;
pub mod gemini_oauth;
pub mod github_copilot;
pub mod github_copilot_auth;
pub mod oauth_helpers;
pub mod openai_codex_provider;
pub mod openai_codex_session;
pub mod prompt;
pub mod provider;
pub mod reasoning;
pub mod registry;
pub mod response_cache;
pub mod retry;
pub mod schema_utils;
pub mod smart_routing;
pub mod token_refreshing;
pub mod util;

#[cfg(test)]
mod codex_test_helpers;

pub mod image_models;
pub mod models;
pub mod reasoning_models;
pub mod vision_models;

#[cfg(feature = "bedrock")]
pub use bedrock::BedrockProvider;
pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerProvider};
pub use codex_chatgpt::CodexChatGptProvider;
pub use config::{
    BedrockConfig, CacheRetention, GeminiOauthConfig, LlmConfig, NearAiConfig, OAUTH_PLACEHOLDER,
    OpenAiCodexConfig, RegistryProviderConfig, SessionConfig,
};
pub use error::LlmError;
pub use failover::{CooldownConfig, FailoverProvider};
pub use gemini_oauth::GeminiOauthProvider;
pub use github_copilot::GithubCopilotProvider;
pub use openai_codex_provider::OpenAiCodexProvider;
pub use openai_codex_session::{OpenAiCodexSession, OpenAiCodexSessionManager};
pub use provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, ImageUrl,
    LlmProvider, ModelMetadata, Role, ToolCall, ToolCompletionRequest, ToolCompletionResponse,
    ToolDefinition, ToolResult, generate_tool_call_id, sanitize_tool_messages,
};
pub use reasoning::{
    ActionPlan, Reasoning, ReasoningContext, RespondOutput, RespondResult, ResponseAnomaly,
    ResponseMetadata, SILENT_REPLY_TOKEN, TOOL_INTENT_NUDGE, TRUNCATED_TOOL_CALL_NOTICE,
    TokenUsage, ToolSelection, is_silent_reply, llm_signals_tool_intent,
};
pub use registry::{ProviderDefinition, ProviderProtocol, ProviderRegistry};
pub use response_cache::{CachedProvider, ResponseCacheConfig};
pub use retry::{RetryConfig, RetryProvider};
pub use smart_routing::{SmartRoutingConfig, SmartRoutingProvider, TaskComplexity};
pub use token_refreshing::TokenRefreshingProvider;
