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
//! - **不提供 `stream_message` dispatch**：当前 ironclaw call site 仅消费同步
//!   `send_message`；流式入口留给 PR-A.3.1（[`OpenAiCompatClient::stream`] 落地后）。
//!
//! [ADR-118 §8.5]: ../../../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md
//! [`OpenAiCompatClient::stream`]: super::openai_compat::OpenAiCompatClient

use crate::error::ApiError;
use crate::providers::anthropic::AnthropicClient;
use crate::providers::openai_compat::OpenAiCompatClient;
use crate::providers::ProviderKind;
use crate::types::{MessageRequest, MessageResponse};

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
