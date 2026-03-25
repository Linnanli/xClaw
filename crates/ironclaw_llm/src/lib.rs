//! LLM integration — shared crate extracted from ironclaw.
//!
//! 核心能力：多提供商 LLM 调用、streaming、tool calling、failover、缓存、成本计算。
//!
//! 支持的后端：
//! - **OpenAI**: 直接 API 或兼容端点（DeepSeek、通义千问、智谱等国产模型）
//! - **Anthropic**: 直接 API
//! - **Ollama**: 本地模型推理
//! - **NEAR AI**: Session token 或 API key 认证（需要 `full` feature）
//! - **AWS Bedrock**: 原生 Converse API（需要 `bedrock` feature）

// ── 核心模块（始终编译）──
pub mod config;
pub mod costs;
pub mod error;
pub mod provider;
pub mod registry;
pub mod retry;
pub mod util;
mod rig_adapter;

// ── 企业级能力（始终编译）──
pub mod circuit_breaker;
pub mod failover;
pub mod response_cache;
pub mod smart_routing;

// ── 推理/模型信息（始终编译）──
mod reasoning;
pub mod image_models;
pub mod reasoning_models;
pub mod vision_models;

// ── 需要 full feature 的模块 ──
#[cfg(feature = "full")]
mod nearai_chat;
#[cfg(feature = "full")]
pub mod session;
#[cfg(feature = "full")]
pub mod models;
#[cfg(feature = "full")]
pub(crate) mod codex_auth;
#[cfg(feature = "full")]
mod codex_chatgpt;
#[cfg(feature = "full")]
mod anthropic_oauth;
#[cfg(feature = "full")]
pub mod oauth_helpers;
#[cfg(feature = "full")]
pub mod recording;

#[cfg(feature = "bedrock")]
mod bedrock;

// ── 公共 re-exports（核心类型，始终可用）──
pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerProvider};
pub use config::{
    BedrockConfig, CacheRetention, LlmConfig, NearAiConfig, OAUTH_PLACEHOLDER,
    RegistryProviderConfig,
};
pub use error::LlmError;
pub use failover::{CooldownConfig, FailoverProvider};
pub use provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, ImageUrl,
    LlmProvider, ModelMetadata, Role, ToolCall, ToolCompletionRequest, ToolCompletionResponse,
    ToolDefinition, ToolResult,
};
pub use reasoning::{
    ActionPlan, Reasoning, ReasoningContext, RespondOutput, RespondResult, SILENT_REPLY_TOKEN,
    TOOL_INTENT_NUDGE, TokenUsage, ToolSelection, is_silent_reply, llm_signals_tool_intent,
};
pub use registry::{ProviderDefinition, ProviderProtocol, ProviderRegistry};
pub use response_cache::{CachedProvider, ResponseCacheConfig};
pub use retry::{RetryConfig, RetryProvider};
pub use rig_adapter::RigAdapter;
pub use smart_routing::{SmartRoutingConfig, SmartRoutingProvider, TaskComplexity};

// full feature re-exports
#[cfg(feature = "full")]
pub use nearai_chat::{ModelInfo, NearAiChatProvider};
#[cfg(feature = "full")]
pub use recording::RecordingLlm;
#[cfg(feature = "full")]
pub use session::{SessionConfig, SessionManager, create_session_manager};

use std::sync::Arc;
use rig::client::CompletionClient;
use secrecy::ExposeSecret;

// ============================================================================
// 核心工厂函数（不依赖 full feature）
// ============================================================================

/// 从 RegistryProviderConfig 创建 LLM provider。
///
/// 这是 Admin Backend 使用的主要入口——从数据库读取模型配置后，
/// 构造 RegistryProviderConfig 并调用此函数创建 provider。
///
/// 支持 OpenAI 兼容（所有国产模型）、Anthropic、Ollama。
pub fn create_provider_from_config(
    config: &RegistryProviderConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    match config.protocol {
        ProviderProtocol::OpenAiCompletions => create_openai_compat_provider(config),
        ProviderProtocol::Anthropic => create_anthropic_provider(config),
        ProviderProtocol::Ollama => create_ollama_provider(config),
    }
}

fn create_openai_compat_provider(
    config: &RegistryProviderConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::openai;

    let mut extra_headers = reqwest::header::HeaderMap::new();
    for (key, value) in &config.extra_headers {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(value),
        ) {
            extra_headers.insert(name, val);
        }
    }

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .unwrap_or_else(|| "no-key".to_string());

    let mut builder = openai::Client::builder().api_key(&api_key);
    if !config.base_url.is_empty() {
        builder = builder.base_url(&config.base_url);
    }
    if !extra_headers.is_empty() {
        builder = builder.http_headers(extra_headers);
    }

    let client: openai::Client = builder.build().map_err(|e| LlmError::RequestFailed {
        provider: config.provider_id.clone(),
        reason: format!("Failed to create OpenAI-compatible client: {e}"),
    })?;

    let client = client.completions_api();
    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = %config.base_url,
        "Using OpenAI-compatible provider"
    );

    let adapter = RigAdapter::new(model, &config.model)
        .with_unsupported_params(config.unsupported_params.clone());
    Ok(Arc::new(adapter))
}

fn create_anthropic_provider(
    config: &RegistryProviderConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::providers::anthropic;

    let api_key = config
        .api_key
        .as_ref()
        .map(|k| k.expose_secret().to_string())
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.provider_id.clone(),
        })?;

    let client: anthropic::Client = if config.base_url.is_empty() {
        anthropic::Client::new(&api_key)
    } else {
        anthropic::Client::builder()
            .api_key(&api_key)
            .base_url(&config.base_url)
            .build()
    }
    .map_err(|e| LlmError::RequestFailed {
        provider: config.provider_id.clone(),
        reason: format!("Failed to create Anthropic client: {e}"),
    })?;

    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        "Using Anthropic provider"
    );

    Ok(Arc::new(
        RigAdapter::new(model, &config.model)
            .with_cache_retention(config.cache_retention)
            .with_unsupported_params(config.unsupported_params.clone()),
    ))
}

fn create_ollama_provider(
    config: &RegistryProviderConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    use rig::client::Nothing;
    use rig::providers::ollama;

    let client: ollama::Client = ollama::Client::builder()
        .base_url(&config.base_url)
        .api_key(Nothing)
        .build()
        .map_err(|e| LlmError::RequestFailed {
            provider: config.provider_id.clone(),
            reason: format!("Failed to create Ollama client: {e}"),
        })?;

    let model = client.completion_model(&config.model);

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        base_url = %config.base_url,
        "Using Ollama provider"
    );

    let adapter = RigAdapter::new(model, &config.model)
        .with_unsupported_params(config.unsupported_params.clone());
    Ok(Arc::new(adapter))
}

// ============================================================================
// full feature 工厂函数（ironclaw 主 crate 使用）
// ============================================================================

#[cfg(feature = "full")]
mod factory {
    use super::*;

    pub async fn create_llm_provider(
        config: &LlmConfig,
        session: Arc<session::SessionManager>,
    ) -> Result<Arc<dyn LlmProvider>, LlmError> {
        let timeout = config.request_timeout_secs;

        if config.backend == "nearai" || config.backend == "near_ai" || config.backend == "near" {
            return create_llm_provider_with_config(&config.nearai, session, timeout);
        }

        if config.backend == "bedrock" {
            #[cfg(feature = "bedrock")]
            {
                return bedrock::BedrockProvider::new(
                    config.bedrock.as_ref().ok_or_else(|| LlmError::AuthFailed {
                        provider: "bedrock".to_string(),
                    })?,
                )
                .await
                .map(|p| Arc::new(p) as Arc<dyn LlmProvider>);
            }
            #[cfg(not(feature = "bedrock"))]
            {
                return Err(LlmError::RequestFailed {
                    provider: "bedrock".to_string(),
                    reason: "Bedrock support not compiled".to_string(),
                });
            }
        }

        let reg_config = config
            .provider
            .as_ref()
            .ok_or_else(|| LlmError::AuthFailed {
                provider: config.backend.clone(),
            })?;

        if reg_config.is_codex_chatgpt {
            let api_key = reg_config.api_key.as_ref().cloned().ok_or_else(|| {
                LlmError::AuthFailed { provider: "codex_chatgpt".to_string() }
            })?;
            let provider = codex_chatgpt::CodexChatGptProvider::with_lazy_model(
                &reg_config.base_url, api_key, &reg_config.model,
                reg_config.refresh_token.clone(), reg_config.auth_path.clone(), timeout,
            );
            return Ok(Arc::new(provider));
        }

        // OAuth Anthropic
        let api_key_is_placeholder = reg_config.api_key.as_ref()
            .is_some_and(|k| k.expose_secret() == config::OAUTH_PLACEHOLDER);
        if reg_config.protocol == ProviderProtocol::Anthropic
            && reg_config.oauth_token.is_some()
            && (reg_config.api_key.is_none() || api_key_is_placeholder)
        {
            let provider = anthropic_oauth::AnthropicOAuthProvider::new(reg_config)?;
            return Ok(Arc::new(provider));
        }

        create_provider_from_config(reg_config)
    }

    pub fn create_llm_provider_with_config(
        config: &NearAiConfig,
        session: Arc<session::SessionManager>,
        request_timeout_secs: u64,
    ) -> Result<Arc<dyn LlmProvider>, LlmError> {
        Ok(Arc::new(nearai_chat::NearAiChatProvider::new_with_timeout(
            config.clone(), session, request_timeout_secs,
        )?))
    }

    pub fn create_cheap_llm_provider(
        config: &LlmConfig,
        session: Arc<session::SessionManager>,
    ) -> Result<Option<Arc<dyn LlmProvider>>, LlmError> {
        let Some(cheap_model) = config.cheap_model_name() else {
            return Ok(None);
        };

        if config.backend == "nearai" {
            let mut cheap_config = config.nearai.clone();
            cheap_config.model = cheap_model.to_string();
            let provider = create_llm_provider_with_config(
                &cheap_config, session, config.request_timeout_secs,
            )?;
            return Ok(Some(provider));
        }

        if config.backend == "bedrock" {
            return Err(LlmError::RequestFailed {
                provider: "bedrock".to_string(),
                reason: "Smart routing not supported for Bedrock".to_string(),
            });
        }

        let reg_config = config.provider.as_ref().ok_or_else(|| LlmError::RequestFailed {
            provider: config.backend.clone(),
            reason: "No registry provider config".to_string(),
        })?;
        let mut cheap_reg = reg_config.clone();
        cheap_reg.model = cheap_model.to_string();
        let provider = create_provider_from_config(&cheap_reg)?;
        Ok(Some(provider))
    }

    pub async fn build_provider_chain(
        config: &LlmConfig,
        session: Arc<session::SessionManager>,
    ) -> Result<(Arc<dyn LlmProvider>, Option<Arc<dyn LlmProvider>>, Option<Arc<recording::RecordingLlm>>), LlmError> {
        let llm = create_llm_provider(config, session.clone()).await?;

        let retry_config = RetryConfig { max_retries: config.nearai.max_retries };
        let llm: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
            Arc::new(RetryProvider::new(llm, retry_config.clone()))
        } else { llm };

        // Smart routing
        let llm: Arc<dyn LlmProvider> = if let Some(cheap_model) = config.cheap_model_name() {
            let cheap = create_cheap_llm_provider(config, session.clone())?
                .ok_or_else(|| LlmError::RequestFailed {
                    provider: config.backend.clone(),
                    reason: format!("Failed to create cheap provider for '{cheap_model}'"),
                })?;
            let cheap: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
                Arc::new(RetryProvider::new(cheap, retry_config.clone()))
            } else { cheap };
            Arc::new(SmartRoutingProvider::new(llm, cheap, SmartRoutingConfig {
                cascade_enabled: config.smart_routing_cascade,
                ..SmartRoutingConfig::default()
            }))
        } else { llm };

        // Failover
        let llm: Arc<dyn LlmProvider> = if let Some(ref fallback_model) = config.nearai.fallback_model {
            let mut fallback_config = config.nearai.clone();
            fallback_config.model = fallback_model.clone();
            let fallback = create_llm_provider_with_config(&fallback_config, session.clone(), config.request_timeout_secs)?;
            let fallback: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
                Arc::new(RetryProvider::new(fallback, retry_config.clone()))
            } else { fallback };
            Arc::new(FailoverProvider::with_cooldown(vec![llm, fallback], CooldownConfig {
                cooldown_duration: std::time::Duration::from_secs(config.nearai.failover_cooldown_secs),
                failure_threshold: config.nearai.failover_cooldown_threshold,
            })?)
        } else { llm };

        // Circuit breaker
        let llm: Arc<dyn LlmProvider> = if let Some(threshold) = config.nearai.circuit_breaker_threshold {
            Arc::new(CircuitBreakerProvider::new(llm, CircuitBreakerConfig {
                failure_threshold: threshold,
                recovery_timeout: std::time::Duration::from_secs(config.nearai.circuit_breaker_recovery_secs),
                ..CircuitBreakerConfig::default()
            }))
        } else { llm };

        // Response cache
        let llm: Arc<dyn LlmProvider> = if config.nearai.response_cache_enabled {
            Arc::new(CachedProvider::new(llm, ResponseCacheConfig {
                ttl: std::time::Duration::from_secs(config.nearai.response_cache_ttl_secs),
                max_entries: config.nearai.response_cache_max_entries,
            }))
        } else { llm };

        // Recording
        let recording_handle = recording::RecordingLlm::from_env(llm.clone());
        let llm: Arc<dyn LlmProvider> = if let Some(ref recorder) = recording_handle {
            Arc::clone(recorder) as Arc<dyn LlmProvider>
        } else { llm };

        let cheap_llm = create_cheap_llm_provider(config, session)?;
        Ok((llm, cheap_llm, recording_handle))
    }
}

#[cfg(feature = "full")]
pub use factory::{
    create_llm_provider, create_llm_provider_with_config,
    create_cheap_llm_provider, build_provider_chain,
};
