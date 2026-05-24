//! Adapter from [`dasclaw_llm_provider::provider::LlmProvider`] to the
//! [`AgentResponder`] trait (ADR-153 step 2 sub-step A2).
//!
//! This is the single bridge between the mature `LlmProvider` ecosystem
//! (Anthropic, OpenAI-compat, Bedrock, GitHub Copilot, Gemini OAuth,
//! ChatGPT, smart routing, response cache, …) and the narrow
//! [`AgentResponder`] seam used by the headless [`Agent`](crate::Agent).
//!
//! ```ignore
//! use std::sync::Arc;
//! use dasclaw_runtime::{Agent, LlmProviderResponder};
//!
//! let provider = Arc::new(my_anthropic_client);
//! let agent = Agent::builder()
//!     .responder(LlmProviderResponder::new(provider))
//!     .system_prompt("You are a helpful assistant.")
//!     .build()?;
//! let text = agent.run("Hello!").await?;
//! ```
//!
//! ## Why a `Box<dyn LlmProvider>` is not exposed
//!
//! `LlmProvider` is already `Send + Sync`, but `Arc<dyn LlmProvider>` is
//! the canonical sharing handle used across the codebase (sessions,
//! routers, caches). The adapter holds an `Arc<P>` where `P: LlmProvider`,
//! so callers keep their existing provider construction code and pay no
//! runtime dispatch cost for the trait call.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::{
    ToolCompletionRequest, ToolCompletionResponse, sanitize_tool_messages,
};
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_llm_provider::provider::provider::LlmProvider;

use crate::agent::AgentResponder;

/// Adapt any [`LlmProvider`] into an [`AgentResponder`].
///
/// `respond` clones the reasoning context's messages, runs
/// [`sanitize_tool_messages`] to drop orphan tool_result blocks (a common
/// source of HTTP 400 from Anthropic), then forwards model override and
/// metadata through a [`ToolCompletionRequest`]. The resulting
/// [`ToolCompletionResponse`] is mapped one-to-one into a
/// [`RespondOutput`] so the agentic loop sees a uniform shape regardless
/// of provider.
pub struct LlmProviderResponder<P: LlmProvider> {
    provider: Arc<P>,
}

impl<P: LlmProvider> LlmProviderResponder<P> {
    /// Wrap a shared provider handle.
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<P: LlmProvider + 'static> AgentResponder for LlmProviderResponder<P> {
    async fn respond(&self, ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        let mut messages = ctx.messages.clone();
        sanitize_tool_messages(&mut messages);

        let mut request = ToolCompletionRequest::new(messages, ctx.available_tools.clone());
        if let Some(model) = ctx.model_override.as_ref() {
            request = request.with_model(model);
        }
        if !ctx.metadata.is_empty() {
            request.metadata = ctx.metadata.clone();
        }

        let response = self.provider.complete_with_tools(request).await?;
        Ok(map_to_respond_output(response))
    }
}

/// Translate a provider [`ToolCompletionResponse`] into the
/// engine-facing [`RespondOutput`].
fn map_to_respond_output(response: ToolCompletionResponse) -> RespondOutput {
    let usage = TokenUsage {
        input_tokens: response.input_tokens,
        output_tokens: response.output_tokens,
        cache_read_input_tokens: response.cache_read_input_tokens,
        cache_creation_input_tokens: response.cache_creation_input_tokens,
    };
    let result = if response.tool_calls.is_empty() {
        RespondResult::Text(response.content.unwrap_or_default())
    } else {
        RespondResult::ToolCalls {
            tool_calls: response.tool_calls,
            content: response.content,
        }
    };
    RespondOutput {
        result,
        usage,
        finish_reason: response.finish_reason,
        metadata: ResponseMetadata::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dasclaw_core::messages::{
        ChatMessage, CompletionRequest, CompletionResponse, FinishReason, ToolCall, ToolDefinition,
    };
    use dasclaw_llm_provider::provider::error::LlmError;
    use rust_decimal::Decimal;
    use std::sync::Mutex;

    /// Mock provider that records the request it was called with and
    /// returns a scripted response.
    struct MockProvider {
        captured: Mutex<Option<ToolCompletionRequest>>,
        response: Mutex<Option<ToolCompletionResponse>>,
    }

    impl MockProvider {
        fn new(response: ToolCompletionResponse) -> Self {
            Self {
                captured: Mutex::new(None),
                response: Mutex::new(Some(response)),
            }
        }

        fn take_request(&self) -> ToolCompletionRequest {
            self.captured
                .lock()
                .unwrap()
                .take()
                .expect("no request captured")
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn model_name(&self) -> &str {
            "mock"
        }

        fn cost_per_token(&self) -> (Decimal, Decimal) {
            (Decimal::ZERO, Decimal::ZERO)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            unreachable!("agent loop only calls complete_with_tools")
        }

        async fn complete_with_tools(
            &self,
            request: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, LlmError> {
            *self.captured.lock().unwrap() = Some(request);
            Ok(self
                .response
                .lock()
                .unwrap()
                .take()
                .expect("response already consumed"))
        }
    }

    fn text_response(text: &str) -> ToolCompletionResponse {
        ToolCompletionResponse {
            content: Some(text.to_string()),
            tool_calls: Vec::new(),
            input_tokens: 10,
            output_tokens: 20,
            finish_reason: FinishReason::Stop,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        }
    }

    fn tool_call_response(name: &str) -> ToolCompletionResponse {
        ToolCompletionResponse {
            content: Some("calling tool".into()),
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: name.into(),
                arguments: serde_json::json!({}),
                reasoning: None,
            }],
            input_tokens: 5,
            output_tokens: 7,
            finish_reason: FinishReason::ToolUse,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_adapter_text_path() {
        let provider = Arc::new(MockProvider::new(text_response("hi there")));
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hello"));

        let output = responder.respond(&mut ctx).await.unwrap();

        match output.result {
            RespondResult::Text(t) => assert_eq!(t, "hi there"),
            _ => panic!("expected Text"),
        }
        assert_eq!(output.usage.input_tokens, 10);
        assert_eq!(output.usage.output_tokens, 20);
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_adapter_forwards_tools_model_metadata() {
        let provider = Arc::new(MockProvider::new(text_response("ok")));
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hi"));
        ctx.available_tools.push(ToolDefinition {
            name: "calc".into(),
            description: "do math".into(),
            parameters: serde_json::json!({"type": "object"}),
        });
        ctx.model_override = Some("claude-opus-4".into());
        ctx.metadata.insert("thread_id".into(), "abc-123".into());

        responder.respond(&mut ctx).await.unwrap();

        let request = provider.take_request();
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, "calc");
        assert_eq!(request.model.as_deref(), Some("claude-opus-4"));
        assert_eq!(
            request.metadata.get("thread_id").map(String::as_str),
            Some("abc-123")
        );
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_a2_adapter_maps_tool_calls() {
        let provider = Arc::new(MockProvider::new(tool_call_response("calc")));
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("compute"));

        let output = responder.respond(&mut ctx).await.unwrap();

        match output.result {
            RespondResult::ToolCalls {
                tool_calls,
                content,
            } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "calc");
                assert_eq!(content.as_deref(), Some("calling tool"));
            }
            _ => panic!("expected ToolCalls"),
        }
        assert_eq!(output.finish_reason, FinishReason::ToolUse);
    }
}
