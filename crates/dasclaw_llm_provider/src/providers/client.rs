//! [`ProviderClient`] —— 跨协议的 enum dispatcher。
//!
//! 把 Anthropic Messages / xAI Chat Completions / OpenAI Chat Completions 三种
//! 协议的 client 统一在一个 enum 下，提供共同的 [`send_message`](ProviderClient::send_message)
//! 入口（同步 `complete` 调用）。本类型**不读取环境变量**，调用方需用具体变体
//! 显式构造，符合 [ADR-118 §8.5] 「无 env 副作用」原则。
//!
//! # 设计差异（vs `claw-code-api::ProviderClient`）
//!
//! - **不提供 `from_model(model)`**：原 `claw-code-api` 的 `from_model` 内部读 env
//!   构造 client，违反纯函数原则。本 crate 让调用方在应用层做 env 采集 + 显式
//!   构造，避免库层与进程环境耦合。
//! - **不提供 `prompt_cache*` / `with_prompt_cache`**：PromptCache 是 `claw-code`
//!   的应用层概念（依赖 telemetry / runtime）；本 crate 保持纯 HTTP client。
//!
//! [ADR-118 §8.5]: ../../../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md
//! [`OpenAiCompatClient::stream`]: super::openai_compat::OpenAiCompatClient

use crate::error::ApiError;
use crate::providers::anthropic::{AnthropicClient, AnthropicStream};
use crate::providers::openai_compat::{OpenAiCompatClient, OpenAiCompatStream};
use crate::providers::ProviderKind;
use crate::types::{MessageRequest, MessageResponse, StreamEvent};

/// 路由到具体协议 client 的 enum。
///
/// 调用方根据模型 / 配置先选定变体，再用 [`send_message`](Self::send_message)
/// 同步发起请求，下层 client 负责 HTTP / 鉴权 / 重试。
#[derive(Debug, Clone)]
pub enum ProviderClient {
    /// Anthropic Messages API client。
    Anthropic(AnthropicClient),
    /// xAI Grok（OpenAI Chat Completions 兼容协议 + 独立 base URL）。
    Xai(OpenAiCompatClient),
    /// OpenAI Chat Completions 协议 client（含 OpenAI 自家、DashScope、Groq、
    /// Kimi、OpenRouter、Tinfoil、Ollama 等所有兼容厂商）。
    OpenAi(OpenAiCompatClient),
}

impl ProviderClient {
    /// 当前 client 走的协议类型。
    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        match self {
            Self::Anthropic(_) => ProviderKind::Anthropic,
            Self::Xai(_) => ProviderKind::Xai,
            Self::OpenAi(_) => ProviderKind::OpenAi,
        }
    }

    /// 同步发送一个消息请求并返回完整响应（非流式）。
    ///
    /// 内部按变体 dispatch 到具体 client 的 `complete` 方法；错误透传 [`ApiError`]
    /// 强类型变体，调用方可精确决策重试 / 重新鉴权。
    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        match self {
            Self::Anthropic(client) => client.complete(request).await,
            Self::Xai(client) | Self::OpenAi(client) => client.complete(request).await,
        }
    }

    /// 流式发送一个消息请求，返回统一的 [`ProviderStream`]，后续可反复调用
    /// [`ProviderStream::next_event`] 增量拉取 [`StreamEvent`]。
    ///
    /// 各后端差异被全部抻在这一层：Anthropic 本身就是 Anthropic-风格事件，
    /// OpenAI / xAI 后端在 [`OpenAiCompatStream`] 内部把 `chat.completion.chunk`
    /// 翻译为 Anthropic 事件 —— 上层 ironclaw 只面对 [`StreamEvent`]。
    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<ProviderStream, ApiError> {
        match self {
            Self::Anthropic(client) => client.stream(request).await.map(ProviderStream::Anthropic),
            Self::Xai(client) => client
                .stream(request)
                .await
                .map(ProviderStream::OpenAiCompat),
            Self::OpenAi(client) => client
                .stream(request)
                .await
                .map(ProviderStream::OpenAiCompat),
        }
    }
}

/// 跨后端统一的流式响应 —— [`ProviderClient::stream_message`] 的返回类型。
///
/// 两个变体均提供同名 `next_event()` 与 `request_id()`，调用方不需区分后端。
#[derive(Debug)]
pub enum ProviderStream {
    /// Anthropic Messages API 原生流。
    Anthropic(AnthropicStream),
    /// OpenAI Chat Completions 兼容流（包含 OpenAI / xAI / DashScope 等）。
    OpenAiCompat(OpenAiCompatStream),
}

impl ProviderStream {
    /// 增量拉取下一个 [`StreamEvent`]；流结束时返回 `Ok(None)`。
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        match self {
            Self::Anthropic(stream) => stream.next_event().await,
            Self::OpenAiCompat(stream) => stream.next_event().await,
        }
    }

    /// 上游返回的 trace ID（如有）。
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Anthropic(stream) => stream.request_id(),
            Self::OpenAiCompat(stream) => stream.request_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::openai_compat::OpenAiCompatConfig;

    #[test]
    fn provider_kind_matches_variant() {
        let anthropic = AnthropicClient::new("sk-ant-test").expect("build anthropic");
        let openai_inner = OpenAiCompatClient::new("sk-openai-test", OpenAiCompatConfig::openai())
            .expect("build openai");
        let xai_inner =
            OpenAiCompatClient::new("xai-test", OpenAiCompatConfig::xai()).expect("build xai");

        assert_eq!(
            ProviderClient::Anthropic(anthropic).provider_kind(),
            ProviderKind::Anthropic
        );
        assert_eq!(
            ProviderClient::OpenAi(openai_inner).provider_kind(),
            ProviderKind::OpenAi
        );
        assert_eq!(
            ProviderClient::Xai(xai_inner).provider_kind(),
            ProviderKind::Xai
        );
    }
}
