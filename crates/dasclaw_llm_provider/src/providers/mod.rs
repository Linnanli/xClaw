//! Provider 路由：模型别名表、provider 类型推断、token 限制查询。
//!
//! 与 `claw-code-api::providers` 的设计差异（ADR-118 §8.5 顺势处理）：
//!
//! - **无 env 副作用**：[`detect_provider_kind`] 接受显式 [`EnvSnapshot`]，**不调** `std::env`。
//!   调用方负责采集环境（一次 `EnvSnapshot::from_process_env()`）后传入，
//!   便于多租户场景把不同租户的环境注入同一进程，也便于测试。
//! - **职责单一**：本模块只做"模型名 → provider 类型"的纯函数映射，不做 HTTP / SSE。
//!   实际 HTTP client 在 PR-A.1+ 落地，路由 metadata 与 client 解耦。

pub mod anthropic;
pub mod client;
pub mod openai_compat;

pub use anthropic::{AnthropicClient, AnthropicStream, AuthSource};
pub use client::ProviderClient;
pub use openai_compat::{OpenAiCompatClient, OpenAiCompatConfig};

/// LLM provider 类型（决定走 Anthropic Messages 协议 / OpenAI Chat Completions 协议 / xAI 协议）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// Anthropic Messages API。
    Anthropic,
    /// xAI Grok（OpenAI Chat Completions 兼容，但 base URL / model 名空间独立）。
    Xai,
    /// OpenAI Chat Completions 协议（含 OpenAI 自家、Kimi、Qwen/DashScope、Groq、OpenRouter、
    /// Tinfoil、Ollama 等所有兼容厂商）。
    OpenAi,
}

/// 单条模型路由元数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderMetadata {
    /// 走哪种协议。
    pub provider: ProviderKind,
    /// 鉴权 env var 名。
    pub auth_env: &'static str,
    /// Base URL override env var 名。
    pub base_url_env: &'static str,
    /// 默认 base URL（当 `base_url_env` 未设置时使用）。
    pub default_base_url: &'static str,
}

/// 模型 token 限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTokenLimit {
    /// 单次响应最大输出 token 数。
    pub max_output_tokens: u32,
    /// 上下文窗口（输入 + 输出）token 总限制。
    pub context_window_tokens: u32,
}

/// 调用方采集的环境快照。无 IO 副作用，便于测试与多租户。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvSnapshot {
    /// `ANTHROPIC_API_KEY` 是否设置（值不重要，只关心是否存在）。
    pub anthropic_api_key_present: bool,
    /// `OPENAI_API_KEY` 是否设置。
    pub openai_api_key_present: bool,
    /// `OPENAI_BASE_URL` 是否设置。
    pub openai_base_url_present: bool,
    /// `XAI_API_KEY` 是否设置。
    pub xai_api_key_present: bool,
    /// `DASHSCOPE_API_KEY` 是否设置。
    pub dashscope_api_key_present: bool,
}

impl EnvSnapshot {
    /// 从当前进程环境采集快照（**唯一**调用 `std::env` 的地方）。
    #[must_use]
    pub fn from_process_env() -> Self {
        Self {
            anthropic_api_key_present: std::env::var_os("ANTHROPIC_API_KEY").is_some(),
            openai_api_key_present: std::env::var_os("OPENAI_API_KEY").is_some(),
            openai_base_url_present: std::env::var_os("OPENAI_BASE_URL").is_some(),
            xai_api_key_present: std::env::var_os("XAI_API_KEY").is_some(),
            dashscope_api_key_present: std::env::var_os("DASHSCOPE_API_KEY").is_some(),
        }
    }
}

// 默认 base URL 常量。集中在此便于审查；client 实现复用同一组常量。
mod base_url {
    pub(super) const ANTHROPIC: &str = "https://api.anthropic.com";
    pub(super) const OPENAI: &str = "https://api.openai.com/v1";
    pub(super) const XAI: &str = "https://api.x.ai/v1";
    pub(super) const DASHSCOPE: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
}

/// 模型别名 → canonical 模型名映射（已小写化的 alias）。
///
/// canonical 名是上游 API 实际接受的模型 ID。Alias 是用户友好缩写。
const MODEL_ALIASES: &[(&str, &str)] = &[
    ("opus", "claude-opus-4-6"),
    ("sonnet", "claude-sonnet-4-6"),
    ("haiku", "claude-haiku-4-5-20251213"),
    ("grok", "grok-3"),
    ("grok-3", "grok-3"),
    ("grok-mini", "grok-3-mini"),
    ("grok-3-mini", "grok-3-mini"),
    ("grok-2", "grok-2"),
    ("kimi", "kimi-k2.5"),
];

/// 解析模型别名 → canonical 模型名。
///
/// 大小写不敏感、去前后空格。无别名时原样返回（去空格）。
#[must_use]
pub fn resolve_model_alias(model: &str) -> String {
    let trimmed = model.trim();
    let lower = trimmed.to_ascii_lowercase();
    MODEL_ALIASES
        .iter()
        .find_map(|(alias, canonical)| (*alias == lower).then_some((*canonical).to_string()))
        .unwrap_or_else(|| trimmed.to_string())
}

/// 根据模型名（canonical 或 alias）推断 provider 元数据。
///
/// 路由优先级：
/// 1. canonical 名前缀匹配（`claude` → Anthropic / `grok` → Xai）
/// 2. provider-namespaced 前缀（`openai/` / `gpt-` → OpenAi / `qwen/` / `qwen-` → DashScope /
///    `kimi/` / `kimi-` → DashScope）
/// 3. 不匹配返回 `None`，由 [`detect_provider_kind`] 走 env-sniff 兜底。
#[must_use]
pub fn metadata_for_model(model: &str) -> Option<ProviderMetadata> {
    let canonical = resolve_model_alias(model);

    if canonical.starts_with("claude") {
        return Some(ProviderMetadata {
            provider: ProviderKind::Anthropic,
            auth_env: "ANTHROPIC_API_KEY",
            base_url_env: "ANTHROPIC_BASE_URL",
            default_base_url: base_url::ANTHROPIC,
        });
    }
    if canonical.starts_with("grok") {
        return Some(ProviderMetadata {
            provider: ProviderKind::Xai,
            auth_env: "XAI_API_KEY",
            base_url_env: "XAI_BASE_URL",
            default_base_url: base_url::XAI,
        });
    }
    // 显式 provider 前缀（`openai/gpt-...` 或 `gpt-...`）。
    // 必须在 env-sniff 之前命中，否则 `ANTHROPIC_API_KEY` 存在时会误路由到 Anthropic。
    if canonical.starts_with("openai/") || canonical.starts_with("gpt-") {
        return Some(ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "OPENAI_API_KEY",
            base_url_env: "OPENAI_BASE_URL",
            default_base_url: base_url::OPENAI,
        });
    }
    // 阿里 DashScope 兼容模式：qwen-* / qwen/* / kimi-* / kimi/* 都走 OpenAI-compat 协议
    // 但 base URL 与 auth env 与原生 OpenAI 不同。
    if canonical.starts_with("qwen/")
        || canonical.starts_with("qwen-")
        || canonical.starts_with("kimi/")
        || canonical.starts_with("kimi-")
    {
        return Some(ProviderMetadata {
            provider: ProviderKind::OpenAi,
            auth_env: "DASHSCOPE_API_KEY",
            base_url_env: "DASHSCOPE_BASE_URL",
            default_base_url: base_url::DASHSCOPE,
        });
    }
    None
}

/// 推断 provider 类型。
///
/// 路由策略（按优先级降序）：
/// 1. 模型名命中 [`metadata_for_model`] → 直接返回对应 [`ProviderKind`]
/// 2. `OPENAI_BASE_URL` + `OPENAI_API_KEY` 都存在 → `OpenAi`
///    （用户显式配置了 OpenAI-compat endpoint，比 Anthropic 兜底优先 ——
///    覆盖 Ollama / vLLM / LM Studio 等本地 model 名无前缀场景）
/// 3. `ANTHROPIC_API_KEY` 存在 → `Anthropic`
/// 4. `OPENAI_API_KEY` 存在 → `OpenAi`
/// 5. `XAI_API_KEY` 存在 → `Xai`
/// 6. 仅 `OPENAI_BASE_URL` 存在（无 key，部分本地 endpoint 不要求 auth）→ `OpenAi`
/// 7. 兜底 → `Anthropic`
#[must_use]
pub fn detect_provider_kind(model: &str, env: &EnvSnapshot) -> ProviderKind {
    if let Some(metadata) = metadata_for_model(model) {
        return metadata.provider;
    }
    if env.openai_base_url_present && env.openai_api_key_present {
        return ProviderKind::OpenAi;
    }
    if env.anthropic_api_key_present {
        return ProviderKind::Anthropic;
    }
    if env.openai_api_key_present {
        return ProviderKind::OpenAi;
    }
    if env.xai_api_key_present {
        return ProviderKind::Xai;
    }
    if env.openai_base_url_present {
        return ProviderKind::OpenAi;
    }
    ProviderKind::Anthropic
}

/// 查询模型 token 限制。
///
/// 仅返回**已知**模型的官方限制；未知模型返回 `None`，由 [`max_tokens_for_model`] 兜底。
#[must_use]
pub fn model_token_limit(model: &str) -> Option<ModelTokenLimit> {
    let canonical = resolve_model_alias(model);
    match canonical.as_str() {
        "claude-opus-4-6" => Some(ModelTokenLimit {
            max_output_tokens: 32_000,
            context_window_tokens: 200_000,
        }),
        "claude-sonnet-4-6" | "claude-haiku-4-5-20251213" => Some(ModelTokenLimit {
            max_output_tokens: 64_000,
            context_window_tokens: 200_000,
        }),
        "grok-3" | "grok-3-mini" => Some(ModelTokenLimit {
            max_output_tokens: 64_000,
            context_window_tokens: 131_072,
        }),
        _ => None,
    }
}

/// 查询模型最大输出 token 数（带兜底）。
///
/// 已知模型走 [`model_token_limit`]；未知模型按 canonical 名包含 `opus` 兜底 32K，否则 64K。
#[must_use]
pub fn max_tokens_for_model(model: &str) -> u32 {
    if let Some(limit) = model_token_limit(model) {
        return limit.max_output_tokens;
    }
    let canonical = resolve_model_alias(model);
    if canonical.contains("opus") {
        32_000
    } else {
        64_000
    }
}

/// 查询模型最大输出 token 数（允许插件/调用方覆盖）。
#[must_use]
pub fn max_tokens_for_model_with_override(model: &str, plugin_override: Option<u32>) -> u32 {
    plugin_override.unwrap_or_else(|| max_tokens_for_model(model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_opus_resolves_to_canonical() {
        assert_eq!(resolve_model_alias("opus"), "claude-opus-4-6");
        assert_eq!(resolve_model_alias("OPUS"), "claude-opus-4-6");
        assert_eq!(resolve_model_alias("  opus  "), "claude-opus-4-6");
    }

    #[test]
    fn alias_sonnet_haiku_resolve() {
        assert_eq!(resolve_model_alias("sonnet"), "claude-sonnet-4-6");
        assert_eq!(resolve_model_alias("haiku"), "claude-haiku-4-5-20251213");
    }

    #[test]
    fn alias_grok_resolves() {
        assert_eq!(resolve_model_alias("grok"), "grok-3");
        assert_eq!(resolve_model_alias("grok-3"), "grok-3");
        assert_eq!(resolve_model_alias("grok-mini"), "grok-3-mini");
        assert_eq!(resolve_model_alias("grok-3-mini"), "grok-3-mini");
        assert_eq!(resolve_model_alias("grok-2"), "grok-2");
    }

    #[test]
    fn alias_kimi_resolves() {
        assert_eq!(resolve_model_alias("kimi"), "kimi-k2.5");
    }

    #[test]
    fn unknown_model_returned_as_is_after_trim() {
        assert_eq!(resolve_model_alias("  custom-model-x  "), "custom-model-x");
    }

    #[test]
    fn metadata_claude_routes_to_anthropic() {
        let m = metadata_for_model("opus").expect("metadata");
        assert_eq!(m.provider, ProviderKind::Anthropic);
        assert_eq!(m.auth_env, "ANTHROPIC_API_KEY");
        assert_eq!(m.default_base_url, "https://api.anthropic.com");
    }

    #[test]
    fn metadata_grok_routes_to_xai() {
        let m = metadata_for_model("grok-3").expect("metadata");
        assert_eq!(m.provider, ProviderKind::Xai);
        assert_eq!(m.auth_env, "XAI_API_KEY");
    }

    #[test]
    fn metadata_openai_namespace_prefix() {
        let m = metadata_for_model("openai/gpt-4.1-mini").expect("metadata");
        assert_eq!(m.provider, ProviderKind::OpenAi);
        assert_eq!(m.auth_env, "OPENAI_API_KEY");
    }

    #[test]
    fn metadata_gpt_prefix_routes_to_openai() {
        let m = metadata_for_model("gpt-4o").expect("metadata");
        assert_eq!(m.provider, ProviderKind::OpenAi);
    }

    #[test]
    fn metadata_qwen_routes_to_dashscope() {
        let m = metadata_for_model("qwen-max").expect("metadata");
        assert_eq!(m.provider, ProviderKind::OpenAi);
        assert_eq!(m.auth_env, "DASHSCOPE_API_KEY");
        assert!(m.default_base_url.contains("dashscope"));
    }

    #[test]
    fn metadata_kimi_alias_routes_to_dashscope() {
        // alias "kimi" 在 MODEL_ALIASES 中解析为 "kimi-k2.5"，命中 kimi- 前缀
        let m = metadata_for_model("kimi").expect("metadata");
        assert_eq!(m.provider, ProviderKind::OpenAi);
        assert_eq!(m.auth_env, "DASHSCOPE_API_KEY");
    }

    #[test]
    fn metadata_unknown_returns_none() {
        assert!(metadata_for_model("custom-model-x").is_none());
    }

    #[test]
    fn detect_recognized_model_short_circuits_env() {
        // 即便 env 全空，已知模型也能走 metadata_for_model 路由
        let env = EnvSnapshot::default();
        assert_eq!(detect_provider_kind("opus", &env), ProviderKind::Anthropic);
        assert_eq!(detect_provider_kind("grok-3", &env), ProviderKind::Xai);
        assert_eq!(detect_provider_kind("gpt-4o", &env), ProviderKind::OpenAi);
    }

    #[test]
    fn detect_unknown_model_with_anthropic_key_falls_to_anthropic() {
        let env = EnvSnapshot {
            anthropic_api_key_present: true,
            ..EnvSnapshot::default()
        };
        assert_eq!(
            detect_provider_kind("custom-x", &env),
            ProviderKind::Anthropic
        );
    }

    #[test]
    fn detect_openai_base_url_plus_key_overrides_anthropic_fallback() {
        // 用户显式配置了 OpenAI-compat endpoint —— 应优先于 Anthropic 兜底
        let env = EnvSnapshot {
            anthropic_api_key_present: true,
            openai_api_key_present: true,
            openai_base_url_present: true,
            ..EnvSnapshot::default()
        };
        assert_eq!(
            detect_provider_kind("qwen2.5-coder:7b", &env),
            ProviderKind::OpenAi
        );
    }

    #[test]
    fn detect_only_openai_base_url_routes_openai() {
        // 本地 endpoint（Ollama 等）不要求 API key
        let env = EnvSnapshot {
            openai_base_url_present: true,
            ..EnvSnapshot::default()
        };
        assert_eq!(
            detect_provider_kind("local-model", &env),
            ProviderKind::OpenAi
        );
    }

    #[test]
    fn detect_xai_key_only() {
        let env = EnvSnapshot {
            xai_api_key_present: true,
            ..EnvSnapshot::default()
        };
        assert_eq!(detect_provider_kind("custom-x", &env), ProviderKind::Xai);
    }

    #[test]
    fn detect_no_env_falls_to_anthropic() {
        let env = EnvSnapshot::default();
        assert_eq!(
            detect_provider_kind("custom-x", &env),
            ProviderKind::Anthropic
        );
    }

    #[test]
    fn token_limit_opus_32k_output_200k_window() {
        let limit = model_token_limit("opus").expect("limit");
        assert_eq!(limit.max_output_tokens, 32_000);
        assert_eq!(limit.context_window_tokens, 200_000);
    }

    #[test]
    fn token_limit_sonnet_haiku_64k_output_200k_window() {
        let s = model_token_limit("sonnet").expect("limit");
        assert_eq!(s.max_output_tokens, 64_000);
        assert_eq!(s.context_window_tokens, 200_000);

        let h = model_token_limit("haiku").expect("limit");
        assert_eq!(h.max_output_tokens, 64_000);
    }

    #[test]
    fn token_limit_grok_64k_output_131k_window() {
        let limit = model_token_limit("grok").expect("limit");
        assert_eq!(limit.max_output_tokens, 64_000);
        assert_eq!(limit.context_window_tokens, 131_072);
    }

    #[test]
    fn token_limit_unknown_returns_none() {
        assert!(model_token_limit("custom-x").is_none());
    }

    #[test]
    fn max_tokens_unknown_opus_substr_falls_to_32k() {
        // 未在 model_token_limit 表中、但 canonical 含 "opus" 字串 → 32K 兜底
        assert_eq!(max_tokens_for_model("custom-opus-future"), 32_000);
    }

    #[test]
    fn max_tokens_unknown_other_falls_to_64k() {
        assert_eq!(max_tokens_for_model("totally-unknown-model"), 64_000);
    }

    #[test]
    fn max_tokens_with_override_uses_override_when_some() {
        assert_eq!(
            max_tokens_for_model_with_override("opus", Some(8_000)),
            8_000
        );
    }

    #[test]
    fn max_tokens_with_override_falls_back_when_none() {
        assert_eq!(max_tokens_for_model_with_override("opus", None), 32_000);
    }
}
