//! LLM provider tree for the ironclaw desktop client.
//!
//! Most wire / HTTP / retry / registry plumbing lives in
//! [`dasclaw_llm_provider::provider`] so the same backend logic powers the
//! desktop client and any future headless agent. This module:
//!
//! * Re-exports the upstream provider tree wholesale so existing in-tree call
//!   sites (`crate::llm::ChatMessage`, `crate::llm::ProviderRegistry`, …) keep
//!   compiling unchanged.
//! * Hosts the application-coupled pieces deliberately kept next to the
//!   desktop client: NEAR AI session manager, recording / replay
//!   interceptors, transcription pipelines, and the high-level provider chain
//!   factory.
//!
//! Per ADR-118 / ADR-129 the provider crate must not depend on
//! `desktop-client/ironclaw`; this module is the split point.

pub use dasclaw_llm_provider::provider::*;

pub mod nearai_chat;
pub mod nearai_embeddings;
pub mod recording;
pub mod session;
pub mod transcription;

pub use nearai_embeddings::NearAiEmbeddings;

pub use nearai_chat::{DEFAULT_MODEL, ModelInfo, NearAiChatProvider, default_models};
pub use recording::{
    ExpectedToolResult, HttpExchange, HttpExchangeRequest, HttpExchangeResponse, HttpInterceptor,
    MemorySnapshotEntry, RecordingHttpInterceptor, RecordingLlm, ReplayingHttpInterceptor,
    RequestHint, TraceFile, TraceResponse, TraceStep, TraceToolCall,
};
pub use session::{SessionConfig, SessionData, SessionManager, create_session_manager};

// ===========================================================================
// Provider factory functions.
//
// These used to live in `dasclaw_llm_provider::provider::mod`; they pull
// together NEAR AI session auth, recording, retry, circuit breaker, smart
// routing, failover, and cache layers into a single provider chain. Because
// they need the application-level `SessionManager`, `NearAiChatProvider`,
// and `RecordingLlm` they live on the desktop-client side of the split.
// ===========================================================================

use std::sync::Arc;

pub use dasclaw_llm_provider::provider::openai_codex_session::OpenAiCodexSessionManager;
use secrecy::ExposeSecret;

#[cfg(feature = "bedrock")]
use dasclaw_llm_provider::provider::bedrock;

/// Create an LLM provider based on configuration.
pub async fn create_llm_provider(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let timeout = config.request_timeout_secs;

    tracing::info!(backend = %config.backend, "Creating LLM provider");

    if config.backend == "nearai" || config.backend == "near_ai" || config.backend == "near" {
        return create_llm_provider_with_config(&config.nearai, session, timeout);
    }

    if config.backend == "gemini_oauth" || config.backend == "gemini-oauth" {
        return create_gemini_oauth_provider(config);
    }

    if config.backend == "bedrock" {
        #[cfg(feature = "bedrock")]
        {
            return create_bedrock_provider(config).await;
        }
        #[cfg(not(feature = "bedrock"))]
        {
            return Err(LlmError::RequestFailed {
                provider: "bedrock".to_string(),
                reason: "Bedrock support not compiled. Rebuild with --features bedrock".to_string(),
            });
        }
    }

    if config.backend == "openai_codex" {
        return Err(LlmError::RequestFailed {
            provider: "openai_codex".to_string(),
            reason:
                "OpenAI Codex uses a dedicated factory path. Use build_provider_chain() instead of create_llm_provider()."
                    .to_string(),
        });
    }

    let reg_config = config
        .provider
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: config.backend.clone(),
        })?;

    create_registry_provider(reg_config, timeout)
}

/// Create an LLM provider from a `NearAiConfig` directly.
pub fn create_llm_provider_with_config(
    config: &NearAiConfig,
    session: Arc<SessionManager>,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let auth_mode = if config.api_key.is_some() {
        "API key"
    } else {
        "session token"
    };
    tracing::debug!(
        model = %config.model,
        base_url = %config.base_url,
        auth = auth_mode,
        timeout_secs = request_timeout_secs,
        "Using NEAR AI (Chat Completions API)"
    );
    Ok(Arc::new(NearAiChatProvider::new_with_timeout(
        config.clone(),
        session,
        request_timeout_secs,
    )?))
}

/// Create a provider from a registry-resolved config.
pub fn create_registry_provider(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    if config.is_codex_chatgpt {
        return create_codex_chatgpt_from_registry(config, request_timeout_secs);
    }

    if matches!(config.protocol, ProviderProtocol::GithubCopilot) {
        let provider = github_copilot::GithubCopilotProvider::new(config, request_timeout_secs)?;
        tracing::debug!(
            provider = %config.provider_id,
            model = %config.model,
            base_url = %config.base_url,
            "Using GitHub Copilot provider (token exchange)"
        );
        return Ok(Arc::new(provider));
    }

    tracing::debug!(
        provider = %config.provider_id,
        model = %config.model,
        protocol = ?config.protocol,
        "Routing provider through ClawCodeLlmProvider"
    );
    claw_code_provider::ClawCodeLlmProvider::from_registry_config(config)
        .map(|p| Arc::new(p) as Arc<dyn LlmProvider>)
}

/// Create an OpenAI-compatible provider from raw parameters.
pub fn create_openai_provider(
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    tracing::debug!(
        base_url = %base_url,
        model = %model,
        key_len = api_key.len(),
        "create_openai_provider: building new provider"
    );
    let config = RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "dynamic".to_string(),
        api_key: Some(secrecy::SecretString::from(api_key.to_string())),
        base_url: base_url.to_string(),
        model: model.to_string(),
        extra_headers: Vec::new(),
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::default(),
        unsupported_params: Vec::new(),
        strict_tools_schema: true,
    };
    claw_code_provider::ClawCodeLlmProvider::from_registry_config(&config)
        .map(|p| Arc::new(p) as Arc<dyn LlmProvider>)
}

fn create_codex_chatgpt_from_registry(
    config: &RegistryProviderConfig,
    request_timeout_secs: u64,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let api_key = config
        .api_key
        .as_ref()
        .cloned()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "codex_chatgpt".to_string(),
        })?;

    tracing::info!(
        configured_model = %config.model,
        base_url = %config.base_url,
        "Using Codex ChatGPT provider (Responses API) — model detection deferred to first call"
    );

    let provider = codex_chatgpt::CodexChatGptProvider::with_lazy_model(
        &config.base_url,
        api_key,
        &config.model,
        config.refresh_token.clone(),
        config.auth_path.clone(),
        request_timeout_secs,
    );

    Ok(Arc::new(provider))
}

#[cfg(feature = "bedrock")]
async fn create_bedrock_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let br = config
        .bedrock
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "bedrock".to_string(),
        })?;

    let provider = bedrock::BedrockProvider::new(br).await?;
    tracing::debug!(
        "Using AWS Bedrock (Converse API, region: {}, model: {})",
        br.region,
        provider.active_model_name(),
    );

    Ok(Arc::new(provider))
}

async fn create_openai_codex_provider(
    config: &LlmConfig,
) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let codex = config
        .openai_codex
        .as_ref()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "openai_codex".to_string(),
        })?;

    let session_mgr = Arc::new(OpenAiCodexSessionManager::new(codex.clone())?);
    session_mgr.ensure_authenticated().await?;

    let token = session_mgr.get_access_token().await?;

    let provider = Arc::new(openai_codex_provider::OpenAiCodexProvider::new(
        &codex.model,
        &codex.api_base_url,
        token.expose_secret(),
        config.request_timeout_secs,
    )?);

    tracing::info!(
        "Using OpenAI Codex (Responses API, model: {}, base: {})",
        codex.model,
        codex.api_base_url,
    );

    Ok(Arc::new(TokenRefreshingProvider::new(
        provider,
        session_mgr,
    )))
}

/// Create a cheap/fast LLM provider for lightweight tasks (heartbeat, routing).
pub fn create_cheap_llm_provider(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<Option<Arc<dyn LlmProvider>>, LlmError> {
    let Some(cheap_model) = config.cheap_model_name() else {
        return Ok(None);
    };

    create_cheap_provider_for_backend(config, session, cheap_model)
}

fn create_cheap_provider_for_backend(
    config: &LlmConfig,
    session: Arc<SessionManager>,
    cheap_model: &str,
) -> Result<Option<Arc<dyn LlmProvider>>, LlmError> {
    if config.backend == "nearai" {
        let mut cheap_config = config.nearai.clone();
        cheap_config.model = cheap_model.to_string();
        let provider =
            create_llm_provider_with_config(&cheap_config, session, config.request_timeout_secs)?;
        return Ok(Some(provider));
    }

    if config.backend == "bedrock" {
        return Err(LlmError::RequestFailed {
            provider: "bedrock".to_string(),
            reason: "Smart routing with cheap model is not supported for Bedrock yet".to_string(),
        });
    }

    if config.backend == "gemini_oauth" {
        let Some(ref gemini_config) = config.gemini_oauth else {
            return Err(LlmError::RequestFailed {
                provider: "gemini_oauth".to_string(),
                reason: "Gemini OAuth config not available for cheap model".to_string(),
            });
        };
        let mut cheap_gemini_config = gemini_config.clone();
        cheap_gemini_config.model = cheap_model.to_string();
        let provider = GeminiOauthProvider::new(cheap_gemini_config)?;
        return Ok(Some(Arc::new(provider)));
    }

    let reg_config = config
        .provider
        .as_ref()
        .ok_or_else(|| LlmError::RequestFailed {
            provider: config.backend.clone(),
            reason: format!(
                "Cannot create cheap provider for backend '{}': no registry provider config available",
                config.backend
            ),
        })?;

    let mut cheap_reg_config = reg_config.clone();
    cheap_reg_config.model = cheap_model.to_string();
    let provider = create_registry_provider(&cheap_reg_config, config.request_timeout_secs)?;
    Ok(Some(provider))
}

/// Build the full LLM provider chain with all configured wrappers.
#[allow(clippy::type_complexity)]
pub async fn build_provider_chain(
    config: &LlmConfig,
    session: Arc<SessionManager>,
) -> Result<
    (
        Arc<dyn LlmProvider>,
        Option<Arc<dyn LlmProvider>>,
        Option<Arc<RecordingLlm>>,
    ),
    LlmError,
> {
    let llm: Arc<dyn LlmProvider> = if config.backend == "openai_codex" {
        create_openai_codex_provider(config).await?
    } else {
        create_llm_provider(config, session.clone()).await?
    };
    tracing::debug!("LLM provider initialized: {}", llm.model_name());

    let retry_config = RetryConfig {
        max_retries: config.nearai.max_retries,
    };
    let llm: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
        tracing::debug!(
            max_retries = retry_config.max_retries,
            "LLM retry wrapper enabled"
        );
        Arc::new(RetryProvider::new(llm, retry_config.clone()))
    } else {
        llm
    };

    let llm: Arc<dyn LlmProvider> = if let Some(cheap_model) = config.cheap_model_name() {
        let cheap = create_cheap_provider_for_backend(config, session.clone(), cheap_model)?
            .ok_or_else(|| LlmError::RequestFailed {
                provider: config.backend.clone(),
                reason: format!(
                    "Failed to create cheap provider for model '{cheap_model}' on backend '{}'",
                    config.backend
                ),
            })?;
        let cheap: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
            Arc::new(RetryProvider::new(cheap, retry_config.clone()))
        } else {
            cheap
        };
        tracing::debug!(
            primary = %llm.model_name(),
            cheap = %cheap.model_name(),
            "Smart routing enabled"
        );
        Arc::new(SmartRoutingProvider::new(
            llm,
            cheap,
            SmartRoutingConfig {
                cascade_enabled: config.smart_routing_cascade,
                ..SmartRoutingConfig::default()
            },
        ))
    } else {
        llm
    };

    let llm: Arc<dyn LlmProvider> = if let Some(ref fallback_model) = config.nearai.fallback_model {
        if fallback_model == &config.nearai.model {
            tracing::warn!(
                "fallback_model is the same as primary model, failover may not be effective"
            );
        }
        let mut fallback_config = config.nearai.clone();
        fallback_config.model = fallback_model.clone();
        let fallback = create_llm_provider_with_config(
            &fallback_config,
            session.clone(),
            config.request_timeout_secs,
        )?;
        tracing::debug!(
            primary = %llm.model_name(),
            fallback = %fallback.model_name(),
            "LLM failover enabled"
        );
        let fallback: Arc<dyn LlmProvider> = if retry_config.max_retries > 0 {
            Arc::new(RetryProvider::new(fallback, retry_config.clone()))
        } else {
            fallback
        };
        let cooldown_config = CooldownConfig {
            cooldown_duration: std::time::Duration::from_secs(config.nearai.failover_cooldown_secs),
            failure_threshold: config.nearai.failover_cooldown_threshold,
        };
        Arc::new(FailoverProvider::with_cooldown(
            vec![llm, fallback],
            cooldown_config,
        )?)
    } else {
        llm
    };

    let llm: Arc<dyn LlmProvider> = if let Some(threshold) = config.nearai.circuit_breaker_threshold
    {
        let cb_config = CircuitBreakerConfig {
            failure_threshold: threshold,
            recovery_timeout: std::time::Duration::from_secs(
                config.nearai.circuit_breaker_recovery_secs,
            ),
            ..CircuitBreakerConfig::default()
        };
        tracing::debug!(
            threshold,
            recovery_secs = config.nearai.circuit_breaker_recovery_secs,
            "LLM circuit breaker enabled"
        );
        Arc::new(CircuitBreakerProvider::new(llm, cb_config))
    } else {
        llm
    };

    let llm: Arc<dyn LlmProvider> = if config.nearai.response_cache_enabled {
        let rc_config = ResponseCacheConfig {
            ttl: std::time::Duration::from_secs(config.nearai.response_cache_ttl_secs),
            max_entries: config.nearai.response_cache_max_entries,
        };
        tracing::debug!(
            ttl_secs = config.nearai.response_cache_ttl_secs,
            max_entries = config.nearai.response_cache_max_entries,
            "LLM response cache enabled"
        );
        Arc::new(CachedProvider::new(llm, rc_config))
    } else {
        llm
    };

    let recording_handle = RecordingLlm::from_env(llm.clone());
    let llm: Arc<dyn LlmProvider> = if let Some(ref recorder) = recording_handle {
        Arc::clone(recorder) as Arc<dyn LlmProvider>
    } else {
        llm
    };

    let cheap_llm = create_cheap_llm_provider(config, session)?;
    if let Some(ref cheap) = cheap_llm {
        tracing::debug!("Cheap LLM provider initialized: {}", cheap.model_name());
    }

    Ok((llm, cheap_llm, recording_handle))
}

/// Build a Gemini OAuth provider from `LlmConfig`.
pub fn create_gemini_oauth_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    let gemini_config = config
        .gemini_oauth
        .clone()
        .ok_or_else(|| LlmError::AuthFailed {
            provider: "gemini_oauth".to_string(),
        })?;
    let provider = GeminiOauthProvider::new(gemini_config)?;
    Ok(Arc::new(provider))
}

/// Build the lightweight `LlmConfig` used by NEAR AI model discovery.
///
/// Reads `NEARAI_AUTH_URL` from the ironclaw environment (defaulting to
/// `https://private.near.ai`). Lives on the desktop-client side because env
/// reads are an application-layer concern (ADR-118 / ADR-129).
pub fn build_nearai_model_fetch_config() -> LlmConfig {
    let auth_base_url = crate::config::helpers::env_or_override("NEARAI_AUTH_URL")
        .unwrap_or_else(|| "https://private.near.ai".to_string());
    let api_key =
        crate::config::helpers::env_or_override("NEARAI_API_KEY").map(secrecy::SecretString::from);
    let base_url_override = crate::config::helpers::env_or_override("NEARAI_BASE_URL");

    LlmConfig {
        backend: "nearai".to_string(),
        session: SessionConfig {
            auth_base_url,
            session_path: crate::config::llm::default_session_path(),
        },
        nearai: NearAiConfig::for_model_discovery(api_key, base_url_override),
        provider: None,
        bedrock: None,
        gemini_oauth: None,
        request_timeout_secs: 120,
        cheap_model: None,
        smart_routing_cascade: false,
        openai_codex: None,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_nearai_config() -> NearAiConfig {
        NearAiConfig {
            model: "test-model".to_string(),
            cheap_model: None,
            base_url: "https://api.near.ai".to_string(),
            api_key: None,
            fallback_model: None,
            max_retries: 3,
            circuit_breaker_threshold: None,
            circuit_breaker_recovery_secs: 30,
            response_cache_enabled: false,
            response_cache_ttl_secs: 3600,
            response_cache_max_entries: 1000,
            failover_cooldown_secs: 300,
            failover_cooldown_threshold: 3,
            smart_routing_cascade: true,
        }
    }

    fn test_llm_config() -> LlmConfig {
        LlmConfig {
            backend: "nearai".to_string(),
            session: SessionConfig::default(),
            nearai: test_nearai_config(),
            provider: None,
            bedrock: None,
            gemini_oauth: None,
            request_timeout_secs: 120,
            cheap_model: None,
            smart_routing_cascade: true,
            openai_codex: None,
        }
    }

    #[test]
    fn test_create_cheap_llm_provider_returns_none_when_not_configured() {
        let config = test_llm_config();
        let session = Arc::new(SessionManager::new(SessionConfig::default()));

        let result = create_cheap_llm_provider(&config, session);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_create_cheap_llm_provider_creates_provider_with_nearai_cheap_model() {
        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("cheap-test-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().model_name(), "cheap-test-model");
    }

    #[test]
    fn test_create_cheap_llm_provider_generic_overrides_nearai() {
        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("nearai-cheap".to_string());
        config.cheap_model = Some("generic-cheap".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert!(provider.is_some());
        assert_eq!(
            provider.unwrap().model_name(),
            "generic-cheap",
            "LLM_CHEAP_MODEL should take priority over NEARAI_CHEAP_MODEL"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_nearai_cheap_ignored_for_non_nearai_backend() {
        let mut config = test_llm_config();
        config.backend = "openai".to_string();
        config.nearai.cheap_model = Some("cheap-test-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "NEARAI_CHEAP_MODEL should be ignored when backend is not nearai"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_bedrock_returns_error() {
        let mut config = test_llm_config();
        config.backend = "bedrock".to_string();
        config.cheap_model = Some("cheap-model".to_string());

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        assert!(
            result.is_err(),
            "Bedrock should return an error for cheap model"
        );
    }

    #[test]
    fn test_create_cheap_llm_provider_gemini_oauth_creates_provider() {
        let mut config = test_llm_config();
        config.backend = "gemini_oauth".to_string();
        config.cheap_model = Some("gemini-2.5-flash-lite".to_string());
        config.gemini_oauth = Some(GeminiOauthConfig {
            model: "gemini-2.5-pro".to_string(),
            credentials_path: std::path::PathBuf::from("/tmp/nonexistent-creds.json"),
        });

        let session = Arc::new(SessionManager::new(SessionConfig::default()));
        let result = create_cheap_llm_provider(&config, session);

        let provider = result.expect("gemini_oauth cheap provider should succeed");
        assert!(provider.is_some(), "Should return Some(provider)");
        assert_eq!(
            provider.unwrap().model_name(),
            "gemini-2.5-flash-lite",
            "Cheap provider should use the overridden model name"
        );
    }

    #[test]
    fn test_cheap_model_name_resolution() {
        let mut config = test_llm_config();
        config.cheap_model = Some("generic".to_string());
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), Some("generic"));

        let mut config = test_llm_config();
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), Some("nearai"));

        let mut config = test_llm_config();
        config.backend = "openai".to_string();
        config.nearai.cheap_model = Some("nearai".to_string());
        assert_eq!(config.cheap_model_name(), None);

        let config = test_llm_config();
        assert_eq!(config.cheap_model_name(), None);
    }
}
