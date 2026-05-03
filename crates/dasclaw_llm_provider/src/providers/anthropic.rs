//! Anthropic Messages API 客户端。
//!
//! 实现 [Anthropic Messages API] 的 `/v1/messages` 端点，提供同步 [`complete`]
//! 与流式 [`stream`] 两种调用模式。基于 [`crate::http::build_http_client_with`]
//! 与 [`crate::sse::SseParser`] 原语。
//!
//! # 与 `claw-code-api::providers::anthropic` 的差异
//!
//! 1. **去除应用层耦合**：本 crate 不依赖 `runtime` / `telemetry` / `prompt_cache`，
//!    `AnthropicClient` 不挂 `SessionTracer` / `PromptCache` 字段，纯 HTTP 客户端。
//!    PromptCache / 会话追踪由上层 ironclaw 在调用方包装。
//! 2. **错误强类型化**：HTTP 状态码精确分发到 `ApiError` 各变体（`RateLimited`、
//!    `AuthFailed`、`ContextWindowExceeded`、`BadRequest`、`ServerError`、`Provider`），
//!    上层免去字符串扫描。
//! 3. **尊重 `Retry-After`**：429 响应携带 `Retry-After` 时优先采纳上游建议（受
//!    `RetryPolicy::max_backoff` 约束）；claw-code 完全忽略此头部。
//! 4. **OAuth 解耦**：本 PR 仅支持 `ApiKey` / `BearerToken` 静态凭据；OAuth flow
//!    由后续 PR 单独承载，避免 client 主体逻辑被凭据轮转撑大。
//!
//! [Anthropic Messages API]: https://docs.anthropic.com/en/api/messages
//! [`complete`]: AnthropicClient::complete
//! [`stream`]: AnthropicClient::stream

use std::collections::VecDeque;
use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::StatusCode;

use crate::error::ApiError;
use crate::retry::{parse_retry_after, RetryPolicy};
use crate::sse::SseParser;
use crate::types::{MessageRequest, MessageResponse, StreamEvent};

const ANTHROPIC_PROVIDER: &str = "anthropic";

/// Anthropic API 默认 base URL。
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Anthropic API 版本号头部值。
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

const REQUEST_ID_HEADER: &str = "request-id";
const ALT_REQUEST_ID_HEADER: &str = "x-request-id";
const RETRY_AFTER_HEADER: &str = "retry-after";

/// Anthropic 客户端鉴权来源。
///
/// 同时携带 `ApiKey` 与 `BearerToken` 时，两者**都会**附加到请求（`x-api-key`
/// 头 + `Authorization: Bearer` 头），与 claw-code 行为一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    /// 无凭据 — 仅用于探针请求或测试 mock 场景。
    None,
    /// `sk-ant-*` 形式 API Key（走 `x-api-key` 头）。
    ApiKey(String),
    /// OAuth bearer token（走 `Authorization: Bearer` 头）。
    BearerToken(String),
    /// 两者都提供（部分企业网关需要双 header）。
    ApiKeyAndBearer {
        /// `sk-ant-*` API Key。
        api_key: String,
        /// OAuth bearer token。
        bearer_token: String,
    },
}

impl AuthSource {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder;
        if let Some(api_key) = self.api_key() {
            builder = builder.header("x-api-key", api_key);
        }
        if let Some(token) = self.bearer_token() {
            builder = builder.bearer_auth(token);
        }
        builder
    }

    fn api_key(&self) -> Option<&str> {
        match self {
            Self::ApiKey(key) | Self::ApiKeyAndBearer { api_key: key, .. } => Some(key),
            Self::None | Self::BearerToken(_) => None,
        }
    }

    fn bearer_token(&self) -> Option<&str> {
        match self {
            Self::BearerToken(token)
            | Self::ApiKeyAndBearer {
                bearer_token: token,
                ..
            } => Some(token),
            Self::None | Self::ApiKey(_) => None,
        }
    }
}

/// Anthropic Messages API 客户端。
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    http: reqwest::Client,
    auth: AuthSource,
    base_url: String,
    retry_policy: RetryPolicy,
}

impl AnthropicClient {
    /// 用 API Key 构造客户端，使用默认 `reqwest::Client` 与默认 [`RetryPolicy`]。
    pub fn new(api_key: impl Into<String>) -> Result<Self, ApiError> {
        Self::with_auth(AuthSource::ApiKey(api_key.into()))
    }

    /// 显式传入 [`AuthSource`]；其余字段取默认。
    pub fn with_auth(auth: AuthSource) -> Result<Self, ApiError> {
        Ok(Self {
            http: crate::http::build_http_client_with(&crate::http::ProxyConfig::default())?,
            auth,
            base_url: DEFAULT_BASE_URL.to_string(),
            retry_policy: RetryPolicy::default(),
        })
    }

    /// 注入预构造的 `reqwest::Client`（例如已配置代理的客户端）。
    #[must_use]
    pub fn with_http_client(mut self, http: reqwest::Client) -> Self {
        self.http = http;
        self
    }

    /// 覆盖 base URL（如自托管网关 / Vertex / Bedrock 兼容前端）。
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// 覆盖默认 [`RetryPolicy`]。
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 当前 base URL（只读）。
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 同步（非流式）发送消息请求。
    ///
    /// 内部强制将 `request.stream` 改写为 `false`。
    pub async fn complete(&self, request: &MessageRequest) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        let response = self.send_with_retry(&request).await?;
        let request_id = request_id_from_headers(response.headers());
        let body = response
            .text()
            .await
            .map_err(|err| ApiError::Transport(err.to_string()))?;
        let mut parsed: MessageResponse =
            serde_json::from_str(&body).map_err(|err| ApiError::Provider {
                provider: ANTHROPIC_PROVIDER.to_string(),
                reason: format!(
                    "failed to deserialize MessageResponse for model={}: {}",
                    request.model, err
                ),
                request_id: request_id.clone(),
            })?;
        if parsed.request_id.is_none() {
            parsed.request_id = request_id;
        }
        Ok(parsed)
    }

    /// 流式发送消息请求；返回 [`AnthropicStream`] 增量推送 SSE 事件。
    ///
    /// 内部强制将 `request.stream` 改写为 `true`。
    pub async fn stream(&self, request: &MessageRequest) -> Result<AnthropicStream, ApiError> {
        let request = MessageRequest {
            stream: true,
            ..request.clone()
        };
        let response = self.send_with_retry(&request).await?;
        let request_id = request_id_from_headers(response.headers());
        Ok(AnthropicStream {
            request_id,
            response,
            parser: SseParser::new().with_context(ANTHROPIC_PROVIDER, request.model.clone()),
            pending: VecDeque::new(),
            done: false,
        })
    }

    async fn send_with_retry(
        &self,
        request: &MessageRequest,
    ) -> Result<reqwest::Response, ApiError> {
        let mut attempt: u32 = 0;
        loop {
            let outcome = self.send_once(request).await;
            match outcome {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let retryable = error.is_retryable();
                    if !retryable || attempt >= self.retry_policy.max_retries {
                        return Err(error);
                    }
                    let suggested = retry_after_from_error(&error);
                    attempt += 1;
                    let delay = self
                        .retry_policy
                        .delay_for_attempt(attempt, suggested)
                        .ok_or_else(|| ApiError::Provider {
                            provider: ANTHROPIC_PROVIDER.to_string(),
                            reason: format!("retry backoff overflow at attempt {attempt}"),
                            request_id: error.request_id().map(ToOwned::to_owned),
                        })?;
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn send_once(&self, request: &MessageRequest) -> Result<reqwest::Response, ApiError> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let builder = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .header("anthropic-version", ANTHROPIC_VERSION);
        let builder = self.auth.apply(builder);
        let response = builder
            .json(request)
            .send()
            .await
            .map_err(|err| ApiError::Transport(err.to_string()))?;
        expect_success(response).await
    }
}

/// Anthropic 流式响应；由 [`AnthropicClient::stream`] 创建。
#[derive(Debug)]
pub struct AnthropicStream {
    request_id: Option<String>,
    response: reqwest::Response,
    parser: SseParser,
    pending: VecDeque<StreamEvent>,
    done: bool,
}

impl AnthropicStream {
    /// 上游返回的 trace ID（如有）。
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// 增量拉取下一个 [`StreamEvent`]；流结束时返回 `Ok(None)`。
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            if self.done {
                let trailing = self.parser.finish()?;
                if trailing.is_empty() {
                    return Ok(None);
                }
                self.pending.extend(trailing);
                continue;
            }
            match self
                .response
                .chunk()
                .await
                .map_err(|err| ApiError::StreamInterrupted(err.to_string()))?
            {
                Some(chunk) => {
                    self.pending.extend(self.parser.push(&chunk)?);
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}

fn request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .or_else(|| headers.get(ALT_REQUEST_ID_HEADER))
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn retry_after_from_error(error: &ApiError) -> Option<Duration> {
    match error {
        ApiError::RateLimited {
            retry_after_secs: Some(secs),
            ..
        } => Some(Duration::from_secs(*secs)),
        _ => None,
    }
}

async fn expect_success(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let request_id = request_id_from_headers(response.headers());
    let retry_after = response
        .headers()
        .get(RETRY_AFTER_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_retry_after);

    let body = response.text().await.unwrap_or_default();
    let parsed_message = parse_anthropic_error_message(&body);
    let reason = parsed_message
        .clone()
        .unwrap_or_else(|| truncate_body(&body));

    Err(map_status_to_api_error(
        status,
        reason,
        retry_after,
        request_id,
    ))
}

fn map_status_to_api_error(
    status: StatusCode,
    reason: String,
    retry_after: Option<Duration>,
    request_id: Option<String>,
) -> ApiError {
    let provider = ANTHROPIC_PROVIDER.to_string();
    match status.as_u16() {
        429 => ApiError::RateLimited {
            provider,
            retry_after_secs: retry_after.map(|d| d.as_secs()),
            request_id,
        },
        401 | 403 => ApiError::AuthFailed {
            provider,
            reason,
            request_id,
        },
        400 => ApiError::BadRequest {
            provider,
            reason,
            request_id,
        },
        408 | 409 | 500 | 502 | 503 | 504 => ApiError::ServerError {
            provider,
            status: status.as_u16(),
            reason,
            request_id,
        },
        _ => ApiError::Provider {
            provider,
            reason: format!("unexpected status {status}: {reason}"),
            request_id,
        },
    }
}

fn parse_anthropic_error_message(body: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct ErrorEnvelope {
        error: ErrorBody,
    }
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        message: String,
    }
    serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .map(|envelope| envelope.error.message)
}

fn truncate_body(body: &str) -> String {
    const MAX: usize = 200;
    if body.len() <= MAX {
        return body.to_string();
    }
    let safe_end = (0..=MAX)
        .rev()
        .find(|&i| body.is_char_boundary(i))
        .unwrap_or(0);
    format!("{}…(truncated, {} bytes)", &body[..safe_end], body.len())
}

#[cfg(test)]
mod tests {
    use super::{
        map_status_to_api_error, parse_anthropic_error_message, AnthropicClient, AuthSource,
    };
    use crate::error::ApiError;
    use reqwest::StatusCode;
    use std::time::Duration;

    #[test]
    fn map_status_429_yields_rate_limited_with_retry_after() {
        let err = map_status_to_api_error(
            StatusCode::TOO_MANY_REQUESTS,
            "slow down".to_string(),
            Some(Duration::from_secs(7)),
            Some("req-1".to_string()),
        );
        match err {
            ApiError::RateLimited {
                provider,
                retry_after_secs,
                request_id,
            } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(retry_after_secs, Some(7));
                assert_eq!(request_id.as_deref(), Some("req-1"));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn map_status_401_403_yields_auth_failed() {
        for code in [401_u16, 403] {
            let err = map_status_to_api_error(
                StatusCode::from_u16(code).unwrap(),
                "bad key".to_string(),
                None,
                None,
            );
            assert!(
                matches!(err, ApiError::AuthFailed { .. }),
                "status {code} should map to AuthFailed: got {err:?}"
            );
        }
    }

    #[test]
    fn map_status_400_yields_bad_request() {
        let err =
            map_status_to_api_error(StatusCode::BAD_REQUEST, "invalid".to_string(), None, None);
        assert!(matches!(err, ApiError::BadRequest { .. }));
    }

    #[test]
    fn map_retryable_5xx_yields_server_error() {
        for code in [500_u16, 502, 503, 504, 408, 409] {
            let err = map_status_to_api_error(
                StatusCode::from_u16(code).unwrap(),
                "boom".to_string(),
                None,
                None,
            );
            match err {
                ApiError::ServerError { status, .. } => assert_eq!(status, code),
                other => panic!("status {code} should map to ServerError: got {other:?}"),
            }
        }
    }

    #[test]
    fn map_unrelated_status_yields_provider_error() {
        let err = map_status_to_api_error(StatusCode::IM_A_TEAPOT, "tea".to_string(), None, None);
        assert!(matches!(err, ApiError::Provider { .. }));
    }

    #[test]
    fn auth_source_apply_attaches_x_api_key() {
        let auth = AuthSource::ApiKey("sk-ant-test".to_string());
        assert_eq!(auth.api_key(), Some("sk-ant-test"));
        assert!(auth.bearer_token().is_none());
    }

    #[test]
    fn auth_source_apply_attaches_bearer_token() {
        let auth = AuthSource::BearerToken("oauth-token".to_string());
        assert_eq!(auth.bearer_token(), Some("oauth-token"));
        assert!(auth.api_key().is_none());
    }

    #[test]
    fn auth_source_apply_attaches_both_headers() {
        let auth = AuthSource::ApiKeyAndBearer {
            api_key: "sk-ant-test".to_string(),
            bearer_token: "oauth-token".to_string(),
        };
        assert_eq!(auth.api_key(), Some("sk-ant-test"));
        assert_eq!(auth.bearer_token(), Some("oauth-token"));
    }

    #[test]
    fn parse_anthropic_error_message_picks_message_field() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"max_tokens too high"}}"#;
        assert_eq!(
            parse_anthropic_error_message(body),
            Some("max_tokens too high".to_string())
        );
    }

    #[test]
    fn parse_anthropic_error_message_returns_none_on_unknown_shape() {
        assert!(parse_anthropic_error_message("not json").is_none());
        assert!(parse_anthropic_error_message("{}").is_none());
    }

    #[test]
    fn anthropic_client_builders_compose_without_panic() {
        let client = AnthropicClient::new("sk-ant-test")
            .expect("client constructible")
            .with_base_url("http://localhost:9999")
            .with_retry_policy(crate::retry::RetryPolicy {
                max_retries: 0,
                ..Default::default()
            });
        assert_eq!(client.base_url(), "http://localhost:9999");
        assert_eq!(client.retry_policy.max_retries, 0);
    }
}
