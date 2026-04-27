//! LLM provider trait.
//!
//! Phase 3 Step D-0：消息领域类型（`ChatMessage`、`ToolCall`、`ContentPart` 等）
//! 已搬到 [`x_claw_agent::messages`]，此处仅保留 provider trait 本身，并通过
//! `pub use` 保持原有模块路径的向后兼容，避免成百上千的 `use crate::llm::...`
//! 需要立即改动。
//!
//! trait 依赖 `rust_decimal::Decimal`（费用计算）和 `LlmError`（provider 错误），
//! 这两者属于 ironclaw 应用层，不下沉到 agent runtime crate。

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::llm::error::LlmError;

// Re-export the agent-runtime domain model so existing `use crate::llm::ChatMessage`
// / `crate::llm::provider::ChatMessage` sites keep working unchanged.
pub use x_claw_agent::messages::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, ImageUrl,
    ModelMetadata, Role, ToolCall, ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
    ToolResult, generate_tool_call_id, sanitize_tool_messages, strip_unsupported_completion_params,
    strip_unsupported_tool_params,
};

/// Trait for LLM providers.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the model name.
    fn model_name(&self) -> &str;

    /// Get cost per token (input, output).
    fn cost_per_token(&self) -> (Decimal, Decimal);

    /// Complete a chat conversation.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Complete with tool use support.
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError>;

    /// List available models from the provider.
    /// Default implementation returns empty list.
    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        Ok(Vec::new())
    }

    /// Fetch metadata for the current model (context length, etc.).
    /// Default returns the model name with no size info.
    async fn model_metadata(&self) -> Result<ModelMetadata, LlmError> {
        Ok(ModelMetadata {
            id: self.model_name().to_string(),
            context_length: None,
        })
    }

    /// Resolve which model should be reported for a given request.
    ///
    /// Providers that ignore per-request model overrides should override this
    /// and return `active_model_name()`.
    fn effective_model_name(&self, requested_model: Option<&str>) -> String {
        requested_model
            .map(std::borrow::ToOwned::to_owned)
            .unwrap_or_else(|| self.active_model_name())
    }

    /// Get the currently active model name.
    ///
    /// May differ from `model_name()` if the model was switched at runtime
    /// via `set_model()`. Default returns `model_name()`.
    fn active_model_name(&self) -> String {
        self.model_name().to_string()
    }

    /// Switch the active model at runtime. Not all providers support this.
    fn set_model(&self, _model: &str) -> Result<(), LlmError> {
        Err(LlmError::RequestFailed {
            provider: "unknown".to_string(),
            reason: "Runtime model switching not supported by this provider".to_string(),
        })
    }

    /// Calculate cost for a completion.
    fn calculate_cost(&self, input_tokens: u32, output_tokens: u32) -> Decimal {
        let (input_cost, output_cost) = self.cost_per_token();
        input_cost * Decimal::from(input_tokens) + output_cost * Decimal::from(output_tokens)
    }

    /// Cost multiplier for cache-creation tokens (Anthropic prompt caching).
    ///
    /// Returns `1.0` by default (no surcharge). Anthropic providers return
    /// `1.25` for 5-minute TTL or `2.0` for 1-hour TTL.
    fn cache_write_multiplier(&self) -> Decimal {
        Decimal::ONE
    }

    /// Discount divisor for cache-read tokens.
    ///
    /// Cached-read cost = `input_rate / cache_read_discount()`.
    /// Returns `1` by default (no discount). Anthropic returns `10` (90% off),
    /// OpenAI would return `2` (50% off).
    fn cache_read_discount(&self) -> Decimal {
        Decimal::ONE
    }

    /// Whether this provider supports streaming text chunks from the LLM API.
    ///
    /// When `true`, [`complete_with_tools_stream`](Self::complete_with_tools_stream)
    /// sends text delta chunks via the provided sender while the response is
    /// being generated, enabling token-level streaming to the frontend.
    ///
    /// Providers backed by structured streaming APIs (OpenAI, Anthropic, Bedrock)
    /// should return `true`. Local-model providers (Ollama) that embed tool calls
    /// as XML in text should return `false` — streaming would break tool-call
    /// recovery which requires the full response.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Streaming variant of [`complete_with_tools`](Self::complete_with_tools).
    ///
    /// Text delta chunks are sent to `chunk_tx` as they arrive from the LLM.
    /// The final [`ToolCompletionResponse`] is returned once the full response
    /// is available (with all tool calls intact).
    ///
    /// Default implementation falls back to the non-streaming path and sends
    /// the complete text as a single chunk.
    async fn complete_with_tools_stream(
        &self,
        request: ToolCompletionRequest,
        chunk_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ToolCompletionResponse, LlmError> {
        let response = self.complete_with_tools(request).await?;
        // Emit the full text as a single chunk for non-streaming providers.
        if let Some(ref content) = response.content {
            let _ = chunk_tx.send(content.clone());
        }
        Ok(response)
    }
}
