//! CLI-side wiring from argv/env → [`dasclaw_runtime::LlmProviderResponder`].
//!
//! ADR-153 §4.4 step 5 follow-up to the step-4 skeleton: prove that
//! [`dasclaw_runtime::Agent`] drives a real provider end-to-end without
//! any desktop dependency. This module exposes:
//!
//! - [`ProviderArgs`] — clap argument group covering the minimum surface
//!   needed to construct a [`ClawCodeLlmProvider`]
//!   ([`ProviderKind`], model, base URL, API key from `--api-key` or env).
//! - [`build_responder`] — pure factory turning [`ProviderArgs`] into a
//!   [`LlmProviderResponder<ClawCodeLlmProvider>`] ready to plug into
//!   [`crate::run`].
//!
//! ## What this is not
//!
//! - Not a config-file loader. The CLI deliberately accepts only argv /
//!   env so it stays a thin smoke driver around the runtime.
//! - Not a credential store. Secrets are taken straight from the
//!   environment and wrapped in [`SecretString`]; persistence belongs in
//!   the desktop client.
//! - Not a router. The CLI picks one provider per invocation; smart
//!   routing, fallback, and circuit breaking live in
//!   `dasclaw_llm_provider::provider` and stay opt-in for callers that
//!   need them.

use std::sync::Arc;

use clap::{Args, ValueEnum};
use dasclaw_llm_provider::provider::claw_code_provider::ClawCodeLlmProvider;
use dasclaw_llm_provider::provider::config::{CacheRetention, RegistryProviderConfig};
use dasclaw_llm_provider::provider::registry::ProviderProtocol;
use dasclaw_runtime::LlmProviderResponder;
use secrecy::SecretString;
use thiserror::Error;

/// Wire protocol selectable from the CLI.
///
/// Mirrors the subset of [`ProviderProtocol`] that the headless smoke
/// driver supports. `GithubCopilot` is intentionally left out because it
/// requires desktop-side token exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderKind {
    /// Anthropic Messages API (`https://api.anthropic.com`).
    Anthropic,
    /// OpenAI Chat Completions API (`https://api.openai.com/v1`).
    Openai,
    /// Any OpenAI-compatible endpoint (xAI, DashScope, Groq, vLLM, …).
    /// `--base-url` is required.
    OpenaiCompat,
    /// Ollama local server (`http://localhost:11434`).
    Ollama,
}

impl ProviderKind {
    fn protocol(self) -> ProviderProtocol {
        match self {
            Self::Anthropic => ProviderProtocol::Anthropic,
            Self::Openai | Self::OpenaiCompat => ProviderProtocol::OpenAiCompletions,
            Self::Ollama => ProviderProtocol::Ollama,
        }
    }

    fn default_base_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com",
            Self::Openai => "https://api.openai.com/v1",
            Self::OpenaiCompat => "",
            Self::Ollama => "http://localhost:11434",
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-3-5-sonnet-latest",
            Self::Openai | Self::OpenaiCompat => "gpt-4o-mini",
            Self::Ollama => "llama3",
        }
    }

    fn requires_api_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Clap argument group covering the minimum surface to build a real
/// LLM-backed responder from the command line.
///
/// Keep this struct flat: it is `#[command(flatten)]`-ed into the binary
/// CLI in [`crate::main`] and into integration test harnesses.
#[derive(Debug, Args, Clone)]
pub struct ProviderArgs {
    /// LLM provider wire protocol.
    #[arg(long, value_enum)]
    pub provider: ProviderKind,

    /// Model identifier (e.g. `claude-3-5-sonnet-latest`, `gpt-4o-mini`).
    /// Defaults depend on `--provider`.
    #[arg(long)]
    pub model: Option<String>,

    /// API endpoint base URL. Required for `openai-compat`; overrides
    /// the protocol default for the other variants.
    #[arg(long)]
    pub base_url: Option<String>,

    /// API key. Read from `--api-key` or, if omitted, from the protocol's
    /// canonical environment variable (`ANTHROPIC_API_KEY`,
    /// `OPENAI_API_KEY`). Ignored for `ollama`.
    #[arg(long, env = "DASCLAW_API_KEY")]
    pub api_key: Option<String>,
}

/// Errors surfaced while assembling a real provider from CLI args.
#[derive(Debug, Error)]
pub enum ProviderBuildError {
    /// `--api-key` was empty and no fallback env var was set.
    #[error(
        "missing API key for provider {provider:?}: pass --api-key or set DASCLAW_API_KEY / {env_hint}"
    )]
    MissingApiKey {
        /// Provider variant the caller selected.
        provider: ProviderKind,
        /// Canonical env var name the user can set as a fallback.
        env_hint: &'static str,
    },

    /// `openai-compat` was selected without `--base-url`.
    #[error("provider 'openai-compat' requires --base-url")]
    MissingBaseUrl,

    /// `dasclaw_llm_provider` rejected the resolved configuration.
    #[error("provider construction failed: {0}")]
    Builder(String),
}

/// Construct a [`LlmProviderResponder`] wrapping a
/// [`ClawCodeLlmProvider`] from the parsed [`ProviderArgs`].
///
/// On success the returned responder is ready for
/// [`dasclaw_runtime::AgentBuilder::responder`]. Network I/O is deferred
/// until the agent actually invokes `respond` — this function is pure
/// modulo `std::env::var` lookups for the per-protocol API-key fallback.
pub fn build_responder(
    args: &ProviderArgs,
) -> Result<LlmProviderResponder<ClawCodeLlmProvider>, ProviderBuildError> {
    let api_key = resolve_api_key(args)?;
    let base_url = resolve_base_url(args)?;
    let model = args
        .model
        .clone()
        .unwrap_or_else(|| args.provider.default_model().to_string());

    let config = RegistryProviderConfig {
        protocol: args.provider.protocol(),
        provider_id: provider_id(args.provider).to_string(),
        api_key,
        base_url,
        model,
        extra_headers: Vec::new(),
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::default(),
        unsupported_params: Vec::new(),
        strict_tools_schema: true,
    };

    let provider = ClawCodeLlmProvider::from_registry_config(&config)
        .map_err(|e| ProviderBuildError::Builder(e.to_string()))?;
    Ok(LlmProviderResponder::new(Arc::new(provider)))
}

fn resolve_api_key(args: &ProviderArgs) -> Result<Option<SecretString>, ProviderBuildError> {
    if !args.provider.requires_api_key() {
        return Ok(None);
    }
    if let Some(key) = args.api_key.as_deref()
        && !key.is_empty()
    {
        return Ok(Some(SecretString::new(key.to_string().into())));
    }
    let env_hint = canonical_env(args.provider);
    if let Ok(key) = std::env::var(env_hint)
        && !key.is_empty()
    {
        return Ok(Some(SecretString::new(key.into())));
    }
    Err(ProviderBuildError::MissingApiKey {
        provider: args.provider,
        env_hint,
    })
}

fn resolve_base_url(args: &ProviderArgs) -> Result<String, ProviderBuildError> {
    match (args.base_url.clone(), args.provider) {
        (Some(url), _) if !url.is_empty() => Ok(url),
        (_, ProviderKind::OpenaiCompat) => Err(ProviderBuildError::MissingBaseUrl),
        (_, kind) => Ok(kind.default_base_url().to_string()),
    }
}

fn provider_id(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::Openai => "openai",
        ProviderKind::OpenaiCompat => "openai_compat",
        ProviderKind::Ollama => "ollama",
    }
}

fn canonical_env(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
        ProviderKind::Openai | ProviderKind::OpenaiCompat => "OPENAI_API_KEY",
        ProviderKind::Ollama => "OLLAMA_API_KEY",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(kind: ProviderKind) -> ProviderArgs {
        ProviderArgs {
            provider: kind,
            model: None,
            base_url: None,
            api_key: Some("test-key".to_string()),
        }
    }

    #[test]
    fn anthropic_defaults_fill_base_url_and_model() {
        let r = build_responder(&args(ProviderKind::Anthropic));
        assert!(r.is_ok(), "{:?}", r.err());
    }

    #[test]
    fn openai_compat_requires_base_url() {
        let mut a = args(ProviderKind::OpenaiCompat);
        a.base_url = None;
        match build_responder(&a) {
            Err(ProviderBuildError::MissingBaseUrl) => {}
            Err(other) => panic!("expected MissingBaseUrl, got {other:?}"),
            Ok(_) => panic!("expected MissingBaseUrl, got Ok"),
        }
    }

    #[test]
    fn ollama_does_not_require_api_key() {
        let mut a = args(ProviderKind::Ollama);
        a.api_key = None;
        let r = build_responder(&a);
        assert!(r.is_ok(), "{:?}", r.err());
    }

    #[test]
    fn missing_api_key_surfaces_env_hint() {
        // Use a process-unique env var name we know is unset, by clearing
        // the canonical one for the duration of the assertion.
        // SAFETY: tests in this module run single-threaded under nextest
        // by default; if that ever changes, switch to a mock layer.
        let saved = std::env::var("ANTHROPIC_API_KEY").ok();
        // SAFETY: scoped restore below.
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };

        let mut a = args(ProviderKind::Anthropic);
        a.api_key = None;
        let err = match build_responder(&a) {
            Err(e) => e,
            Ok(_) => panic!("expected MissingApiKey, got Ok"),
        };

        if let Some(v) = saved {
            // SAFETY: scoped restore of pre-existing value.
            unsafe { std::env::set_var("ANTHROPIC_API_KEY", v) };
        }

        match err {
            ProviderBuildError::MissingApiKey { env_hint, .. } => {
                assert_eq!(env_hint, "ANTHROPIC_API_KEY");
            }
            other => panic!("expected MissingApiKey, got {other:?}"),
        }
    }
}
