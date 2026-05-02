//! 强类型化 API error。
//!
//! 与 `claw-code-api::ApiError` 的设计差异（ADR-118 §8.5 架构债 #2 顺势处理）：
//!
//! - **拆变体**：`RateLimited` / `AuthFailed` / `ContextWindowExceeded` 等单独成变体，
//!   调用方可精确决策（Retry-After 退避 / 触发 OAuth re-login / 不重试）。
//! - **保留 trace ID**：`Provider` 变体保留 `request_id` 字段供日志关联。
//! - **`Display` 不暴露密钥**：实现 `Display` 时只输出错误描述 + trace ID + HTTP 状态，
//!   不会序列化原始 API key 或鉴权 header（即便错误对象内嵌了原始上下文也不输出）。

use thiserror::Error;

/// 与上游 LLM provider HTTP 调用相关的错误。
#[derive(Debug, Error)]
pub enum ApiError {
    /// 速率限制（HTTP 429）。`retry_after_secs` 来自上游 `Retry-After` header；
    /// 缺失时上层应使用指数退避默认值。
    #[error(
        "rate limited by {provider}{}",
        retry_after_secs.map_or_else(String::new, |s| format!(" (retry after {s}s)"))
    )]
    RateLimited {
        /// 上游 provider 标识（"anthropic" / "openai" / "xai" / "dashscope" 等）。
        provider: String,
        /// 上游建议的等待秒数（`Retry-After` header）。
        retry_after_secs: Option<u64>,
        /// 上游 trace ID（如有）。
        request_id: Option<String>,
    },

    /// 鉴权失败（HTTP 401 / 403）。调用方应触发重新登录或刷新 token，**不应**直接重试。
    #[error("authentication failed for {provider}: {reason}")]
    AuthFailed {
        /// 上游 provider 标识。
        provider: String,
        /// 失败原因（不含密钥）。
        reason: String,
        /// 上游 trace ID（如有）。
        request_id: Option<String>,
    },

    /// 上下文窗口超限。这是**不可重试**错误 —— 必须由上层裁剪上下文后重发。
    #[error(
        "context window exceeded for {model}: \
         estimated {estimated_total_tokens} tokens (input {estimated_input_tokens} + \
         requested output {requested_output_tokens}) > limit {context_window_tokens}"
    )]
    ContextWindowExceeded {
        /// canonical 模型名。
        model: String,
        /// 估算的输入 token 数。
        estimated_input_tokens: u32,
        /// 请求的输出 token 数。
        requested_output_tokens: u32,
        /// 估算的总 token 数。
        estimated_total_tokens: u32,
        /// 模型上下文窗口限制。
        context_window_tokens: u32,
    },

    /// 请求体不合法（HTTP 400）。通常是 client 实现 bug 或上游协议变更。
    #[error("bad request to {provider}: {reason}")]
    BadRequest {
        /// 上游 provider 标识。
        provider: String,
        /// 失败原因。
        reason: String,
        /// 上游 trace ID（如有）。
        request_id: Option<String>,
    },

    /// 上游服务端错误（HTTP 5xx）。上层可重试。
    #[error("server error from {provider} (HTTP {status}): {reason}")]
    ServerError {
        /// 上游 provider 标识。
        provider: String,
        /// HTTP 状态码。
        status: u16,
        /// 失败原因。
        reason: String,
        /// 上游 trace ID（如有）。
        request_id: Option<String>,
    },

    /// 其他 provider 端错误（不属于上述任何分类）。
    #[error("provider error from {provider}: {reason}")]
    Provider {
        /// 上游 provider 标识。
        provider: String,
        /// 失败原因。
        reason: String,
        /// 上游 trace ID（如有）。
        request_id: Option<String>,
    },

    /// 传输层错误（网络断开、TLS 失败、连接超时等）。
    #[error("transport error: {0}")]
    Transport(String),

    /// JSON 序列化 / 反序列化错误。
    #[error("serialization error: {0}")]
    Serialization(String),

    /// 流式响应被异常中断（连接中途断开、SSE 协议错误）。
    #[error("stream interrupted: {0}")]
    StreamInterrupted(String),

    /// SSE 帧解析错误（payload 不是合法 SSE 帧 / 字段格式错误）。
    #[error("malformed SSE frame: {0}")]
    MalformedSseFrame(String),
}

impl ApiError {
    /// 判定错误是否应触发重试。
    ///
    /// **可重试**：`RateLimited` / `ServerError` / `Transport` / `StreamInterrupted`。
    /// **不可重试**：`AuthFailed`（应重新登录）/ `ContextWindowExceeded`（应裁剪上下文）/
    /// `BadRequest`（client 端 bug）/ `Serialization` / `MalformedSseFrame` / `Provider`（语义未知）。
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::ServerError { .. }
                | Self::Transport(_)
                | Self::StreamInterrupted(_)
        )
    }

    /// 获取 trace ID（如有）。便于上层日志 / 用户报错关联。
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::RateLimited { request_id, .. }
            | Self::AuthFailed { request_id, .. }
            | Self::BadRequest { request_id, .. }
            | Self::ServerError { request_id, .. }
            | Self::Provider { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// 获取 provider 标识（如有）。
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        match self {
            Self::RateLimited { provider, .. }
            | Self::AuthFailed { provider, .. }
            | Self::BadRequest { provider, .. }
            | Self::ServerError { provider, .. }
            | Self::Provider { provider, .. } => Some(provider.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_is_retryable() {
        let err = ApiError::RateLimited {
            provider: "anthropic".into(),
            retry_after_secs: Some(30),
            request_id: Some("req_xyz".into()),
        };
        assert!(err.is_retryable());
        assert_eq!(err.provider(), Some("anthropic"));
        assert_eq!(err.request_id(), Some("req_xyz"));
    }

    #[test]
    fn auth_failed_is_not_retryable() {
        let err = ApiError::AuthFailed {
            provider: "openai".into(),
            reason: "invalid api key".into(),
            request_id: None,
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn context_window_exceeded_is_not_retryable() {
        let err = ApiError::ContextWindowExceeded {
            model: "claude-opus-4-6".into(),
            estimated_input_tokens: 200_000,
            requested_output_tokens: 4000,
            estimated_total_tokens: 204_000,
            context_window_tokens: 200_000,
        };
        assert!(!err.is_retryable());
        assert_eq!(err.provider(), None);
    }

    #[test]
    fn server_error_is_retryable() {
        let err = ApiError::ServerError {
            provider: "anthropic".into(),
            status: 503,
            reason: "service unavailable".into(),
            request_id: None,
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn bad_request_not_retryable() {
        let err = ApiError::BadRequest {
            provider: "openai".into(),
            reason: "max_tokens required".into(),
            request_id: None,
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn transport_is_retryable() {
        let err = ApiError::Transport("connection reset".into());
        assert!(err.is_retryable());
        assert_eq!(err.provider(), None);
        assert_eq!(err.request_id(), None);
    }

    #[test]
    fn serialization_is_not_retryable() {
        let err = ApiError::Serialization("expected `,` at line 3".into());
        assert!(!err.is_retryable());
    }

    #[test]
    fn rate_limited_display_includes_retry_after() {
        let err = ApiError::RateLimited {
            provider: "anthropic".into(),
            retry_after_secs: Some(30),
            request_id: None,
        };
        let msg = format!("{err}");
        assert!(msg.contains("rate limited by anthropic"));
        assert!(msg.contains("retry after 30s"));
    }

    #[test]
    fn rate_limited_display_omits_retry_after_when_none() {
        let err = ApiError::RateLimited {
            provider: "openai".into(),
            retry_after_secs: None,
            request_id: None,
        };
        let msg = format!("{err}");
        assert_eq!(msg, "rate limited by openai");
    }

    #[test]
    fn context_window_display_format_is_actionable() {
        let err = ApiError::ContextWindowExceeded {
            model: "claude-opus-4-6".into(),
            estimated_input_tokens: 198_000,
            requested_output_tokens: 4_000,
            estimated_total_tokens: 202_000,
            context_window_tokens: 200_000,
        };
        let msg = format!("{err}");
        assert!(msg.contains("198000 + requested output 4000"));
        assert!(msg.contains("> limit 200000"));
    }
}
