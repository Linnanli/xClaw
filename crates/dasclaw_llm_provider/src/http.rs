//! HTTP transport 层 — `reqwest::Client` 构造 + 代理配置。
//!
//! 本模块在 [ADR-118] 定义下，为后续 PR-A.2 / PR-A.3 的 `AnthropicClient` /
//! `OpenAiCompatClient` 提供共享的传输层基础。
//!
//! # 与 `claw-code-api::http_client` 的差异
//!
//! 1. **纯函数化**：`ProxyConfig::from_lookup` 接受闭包查询，不直接调用
//!    `std::env::var`。`ProxyConfig::from_process_env` 是显式命名的副作用入口，
//!    便于上层多租户 / 测试隔离。
//! 2. **错误统一映射**：`reqwest` 错误统一映射为 [`ApiError::Transport`]，
//!    上游 `claw-code-api::ApiError::Http` 是独立变体。我们不引入 `Http` 变体，
//!    保持 `ApiError` 的语义简洁（10 个变体 → 不增加）。
//!
//! [ADR-118]: ../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md

use crate::error::ApiError;

const HTTP_PROXY_KEYS: [&str; 2] = ["HTTP_PROXY", "http_proxy"];
const HTTPS_PROXY_KEYS: [&str; 2] = ["HTTPS_PROXY", "https_proxy"];
const NO_PROXY_KEYS: [&str; 2] = ["NO_PROXY", "no_proxy"];

/// 出站 HTTP 客户端的代理环境快照。
///
/// 当 `proxy_url` 设置时，作为 HTTP/HTTPS 流量的 catch-all 代理，优先级高于
/// 按 scheme 分别设置的 `http_proxy` / `https_proxy` 字段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyConfig {
    /// HTTP 流量代理 URL（来自 `HTTP_PROXY` / `http_proxy`）。
    pub http_proxy: Option<String>,
    /// HTTPS 流量代理 URL（来自 `HTTPS_PROXY` / `https_proxy`）。
    pub https_proxy: Option<String>,
    /// 不走代理的 host 列表（来自 `NO_PROXY` / `no_proxy`）。
    pub no_proxy: Option<String>,
    /// 显式配置的统一代理 URL；优先级高于 `http_proxy` / `https_proxy`。
    pub proxy_url: Option<String>,
}

impl ProxyConfig {
    /// 从进程环境变量读取代理配置（**有副作用**，仅供应用启动期使用）。
    ///
    /// 测试 / 多租户场景请改用 [`ProxyConfig::from_lookup`] 或
    /// 直接构造结构体字段。
    #[must_use]
    pub fn from_process_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// 从纯 lookup 闭包读取代理配置；不触碰进程环境。
    ///
    /// 同一 scheme 优先大写、再回退小写。空字符串视作未设置。
    pub fn from_lookup<F>(mut lookup: F) -> Self
    where
        F: FnMut(&str) -> Option<String>,
    {
        Self {
            http_proxy: first_non_empty(&HTTP_PROXY_KEYS, &mut lookup),
            https_proxy: first_non_empty(&HTTPS_PROXY_KEYS, &mut lookup),
            no_proxy: first_non_empty(&NO_PROXY_KEYS, &mut lookup),
            proxy_url: None,
        }
    }

    /// 从单一 URL 构造统一代理配置；HTTP 与 HTTPS 共用同一目标。
    #[must_use]
    pub fn from_proxy_url(url: impl Into<String>) -> Self {
        Self {
            proxy_url: Some(url.into()),
            ..Self::default()
        }
    }

    /// 是否未配置任何代理。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.proxy_url.is_none() && self.http_proxy.is_none() && self.https_proxy.is_none()
    }
}

fn first_non_empty<F>(keys: &[&str], lookup: &mut F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    keys.iter()
        .find_map(|key| lookup(key).filter(|value| !value.is_empty()))
}

/// 使用进程环境构造 `reqwest::Client`。
///
/// 仅 `HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY` 任一未设置或为空时，行为与
/// `reqwest::Client::new()` 一致。
///
/// **副作用**：调用 `std::env::var`。多租户 / 测试请改用
/// [`build_http_client_with`]。
pub fn build_http_client() -> Result<reqwest::Client, ApiError> {
    build_http_client_with(&ProxyConfig::from_process_env())
}

/// 显式 [`ProxyConfig`] 版本；不读进程环境，**纯函数**。
///
/// `proxy_url` 字段优先级高于 `http_proxy` / `https_proxy`：当 `proxy_url` 设
/// 置时，HTTP 与 HTTPS 流量都走它。
pub fn build_http_client_with(config: &ProxyConfig) -> Result<reqwest::Client, ApiError> {
    let mut builder = reqwest::Client::builder().no_proxy();

    let no_proxy = config
        .no_proxy
        .as_deref()
        .and_then(reqwest::NoProxy::from_string);

    let (http_proxy_url, https_url) = match config.proxy_url.as_deref() {
        Some(unified) => (Some(unified), Some(unified)),
        None => (config.http_proxy.as_deref(), config.https_proxy.as_deref()),
    };

    if let Some(url) = https_url {
        let mut proxy = reqwest::Proxy::https(url).map_err(transport_error)?;
        if let Some(filter) = no_proxy.clone() {
            proxy = proxy.no_proxy(Some(filter));
        }
        builder = builder.proxy(proxy);
    }

    if let Some(url) = http_proxy_url {
        let mut proxy = reqwest::Proxy::http(url).map_err(transport_error)?;
        if let Some(filter) = no_proxy.clone() {
            proxy = proxy.no_proxy(Some(filter));
        }
        builder = builder.proxy(proxy);
    }

    builder.build().map_err(transport_error)
}

fn transport_error(err: reqwest::Error) -> ApiError {
    ApiError::Transport(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{build_http_client_with, ProxyConfig};

    fn config_from_map(pairs: &[(&str, &str)]) -> ProxyConfig {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        ProxyConfig::from_lookup(|key| map.get(key).cloned())
    }

    #[test]
    fn proxy_config_is_empty_when_no_env_vars_are_set() {
        let config = config_from_map(&[]);
        assert!(config.is_empty());
        assert_eq!(config, ProxyConfig::default());
    }

    #[test]
    fn proxy_config_reads_uppercase_http_https_and_no_proxy() {
        let pairs = [
            ("HTTP_PROXY", "http://proxy.internal:3128"),
            ("HTTPS_PROXY", "http://secure.internal:3129"),
            ("NO_PROXY", "localhost,127.0.0.1,.corp"),
        ];
        let config = config_from_map(&pairs);
        assert_eq!(
            config.http_proxy.as_deref(),
            Some("http://proxy.internal:3128")
        );
        assert_eq!(
            config.https_proxy.as_deref(),
            Some("http://secure.internal:3129")
        );
        assert_eq!(
            config.no_proxy.as_deref(),
            Some("localhost,127.0.0.1,.corp")
        );
        assert!(!config.is_empty());
    }

    #[test]
    fn proxy_config_falls_back_to_lowercase_keys() {
        let pairs = [
            ("http_proxy", "http://lower.internal:3128"),
            ("https_proxy", "http://lower-secure.internal:3129"),
            ("no_proxy", ".lower"),
        ];
        let config = config_from_map(&pairs);
        assert_eq!(
            config.http_proxy.as_deref(),
            Some("http://lower.internal:3128")
        );
        assert_eq!(
            config.https_proxy.as_deref(),
            Some("http://lower-secure.internal:3129")
        );
        assert_eq!(config.no_proxy.as_deref(), Some(".lower"));
    }

    #[test]
    fn proxy_config_prefers_uppercase_over_lowercase_when_both_set() {
        let pairs = [
            ("HTTP_PROXY", "http://upper.internal:3128"),
            ("http_proxy", "http://lower.internal:3128"),
        ];
        let config = config_from_map(&pairs);
        assert_eq!(
            config.http_proxy.as_deref(),
            Some("http://upper.internal:3128")
        );
    }

    #[test]
    fn proxy_config_treats_empty_strings_as_unset() {
        let pairs = [("HTTP_PROXY", ""), ("http_proxy", "")];
        let config = config_from_map(&pairs);
        assert!(config.http_proxy.is_none());
    }

    #[test]
    fn build_http_client_succeeds_when_no_proxy_is_configured() {
        let config = ProxyConfig::default();
        let result = build_http_client_with(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn build_http_client_succeeds_with_valid_http_and_https_proxies() {
        let config = ProxyConfig {
            http_proxy: Some("http://proxy.internal:3128".to_string()),
            https_proxy: Some("http://secure.internal:3129".to_string()),
            no_proxy: Some("localhost,127.0.0.1".to_string()),
            proxy_url: None,
        };
        let result = build_http_client_with(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn build_http_client_returns_transport_error_for_invalid_proxy_url() {
        let config = ProxyConfig {
            http_proxy: None,
            https_proxy: Some("not a url".to_string()),
            no_proxy: None,
            proxy_url: None,
        };
        let result = build_http_client_with(&config);
        let error = result.expect_err("invalid proxy URL must be reported as a build failure");
        assert!(
            matches!(error, crate::error::ApiError::Transport(_)),
            "expected ApiError::Transport for invalid proxy URL, got: {error:?}"
        );
    }

    #[test]
    fn from_proxy_url_sets_unified_field_and_leaves_per_scheme_empty() {
        let config = ProxyConfig::from_proxy_url("http://unified.internal:3128");
        assert_eq!(
            config.proxy_url.as_deref(),
            Some("http://unified.internal:3128")
        );
        assert!(config.http_proxy.is_none());
        assert!(config.https_proxy.is_none());
        assert!(!config.is_empty());
    }

    #[test]
    fn build_http_client_succeeds_with_unified_proxy_url() {
        let config = ProxyConfig {
            proxy_url: Some("http://unified.internal:3128".to_string()),
            no_proxy: Some("localhost".to_string()),
            ..ProxyConfig::default()
        };
        let result = build_http_client_with(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn proxy_url_takes_precedence_over_per_scheme_fields() {
        let config = ProxyConfig {
            http_proxy: Some("http://per-scheme.internal:1111".to_string()),
            https_proxy: Some("http://per-scheme.internal:2222".to_string()),
            no_proxy: None,
            proxy_url: Some("http://unified.internal:3128".to_string()),
        };
        let result = build_http_client_with(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn build_http_client_returns_transport_error_for_invalid_unified_proxy_url() {
        let config = ProxyConfig::from_proxy_url("not a url");
        let result = build_http_client_with(&config);
        assert!(
            matches!(result, Err(crate::error::ApiError::Transport(_))),
            "invalid unified proxy URL should fail: {result:?}"
        );
    }
}
