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
use dasclaw_llm_provider::provider::reasoning::clean_user_visible_response;
use tokio::sync::mpsc;

use crate::agent::{AgentEvent, AgentResponder};

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
        let request = build_request(ctx);
        let response = self.provider.complete_with_tools(request).await?;
        Ok(clean_respond_output(map_to_respond_output(response)))
    }

    /// Forward token-level text deltas to `event_tx` while the
    /// underlying provider streams the response (issue #908).
    ///
    /// We bridge the provider's `UnboundedSender<String>` chunk channel
    /// onto the agent-level `mpsc::Sender<AgentEvent>` via a forwarder
    /// task: the LLM call only finishes once the provider closes its
    /// chunk sender, so the forwarder always drains to completion. A
    /// closed `event_tx` (consumer hung up) is treated as best-effort —
    /// we keep draining the LLM chunks to avoid stalling the provider's
    /// underlying SSE stream.
    async fn respond_streaming(
        &self,
        ctx: &mut ReasoningContext,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RespondOutput, HostError> {
        let request = build_request(ctx);
        if !self.provider.supports_streaming() {
            let response = self.provider.complete_with_tools(request).await?;
            send_response_events(
                &event_tx,
                response.reasoning.as_deref(),
                response.content.as_deref(),
            )
            .await;
            return Ok(clean_respond_output(map_to_respond_output(response)));
        }

        let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<String>();
        let reasoning_event_tx = event_tx.clone();

        // Forwarder runs concurrently with `complete_with_tools_stream`:
        // the provider call won't return until it closes `chunk_tx`, at
        // which point `chunk_rx` yields `None` and the forwarder exits.
        let forwarder = tokio::spawn(async move {
            let mut stream = LegacyReasoningStream::default();
            while let Some(chunk) = chunk_rx.recv().await {
                for event in stream.push(&chunk) {
                    if event_tx.send(event).await.is_err() {
                        // Consumer hung up — drain the rest silently so
                        // the provider stream isn't back-pressured.
                        while chunk_rx.recv().await.is_some() {}
                        return;
                    }
                }
            }
            for event in stream.finish() {
                if event_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        let response = self
            .provider
            .complete_with_tools_stream(request, chunk_tx)
            .await;
        // Whether the call succeeded or failed, wait for the forwarder
        // to drain so all already-sent chunks reach `event_tx` before
        // the caller observes the FinishReason event in `call_llm`.
        let _ = forwarder.await;
        if let Ok(response) = &response {
            send_reasoning_event(&reasoning_event_tx, response.reasoning.as_deref()).await;
        }
        Ok(clean_respond_output(map_to_respond_output(response?)))
    }
}

async fn send_response_events(
    event_tx: &mpsc::Sender<AgentEvent>,
    reasoning: Option<&str>,
    content: Option<&str>,
) {
    send_reasoning_event(event_tx, reasoning).await;

    let Some(content) = content else {
        return;
    };
    let mut stream = LegacyReasoningStream::default();
    for event in stream.push(content).into_iter().chain(stream.finish()) {
        if event_tx.send(event).await.is_err() {
            break;
        }
    }
}

async fn send_reasoning_event(event_tx: &mpsc::Sender<AgentEvent>, reasoning: Option<&str>) {
    let Some(reasoning) = reasoning else {
        return;
    };
    if reasoning.is_empty() {
        return;
    }
    let _ = event_tx
        .send(AgentEvent::ReasoningSummaryChunk(reasoning.to_string()))
        .await;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyReasoningMode {
    Text,
    Reasoning,
    Final,
}

#[derive(Debug)]
struct LegacyReasoningStream {
    buffer: String,
    mode: LegacyReasoningMode,
    saw_final: bool,
}

impl Default for LegacyReasoningStream {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            mode: LegacyReasoningMode::Text,
            saw_final: false,
        }
    }
}

impl LegacyReasoningStream {
    fn push(&mut self, chunk: &str) -> Vec<AgentEvent> {
        self.buffer.push_str(chunk);
        self.drain(false)
    }

    fn finish(&mut self) -> Vec<AgentEvent> {
        self.drain(true)
    }

    fn drain(&mut self, finish: bool) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        loop {
            let tags = find_tag_spans(&self.buffer);
            match self.mode {
                LegacyReasoningMode::Text => {
                    let Some(tag) = tags.iter().find(|tag| {
                        matches!(tag.kind, TagKind::OpenReasoning | TagKind::OpenFinal)
                    }) else {
                        let safe_len = safe_text_prefix_len(&self.buffer, finish);
                        if safe_len == 0 {
                            break;
                        }
                        let text = self.buffer.drain(..safe_len).collect::<String>();
                        if !self.saw_final && !text.is_empty() {
                            events.push(AgentEvent::TextChunk(text));
                        }
                        continue;
                    };
                    if tag.start > 0 {
                        let text = self.buffer.drain(..tag.start).collect::<String>();
                        if !self.saw_final && !text.is_empty() {
                            events.push(AgentEvent::TextChunk(text));
                        }
                    }
                    let Some(tag) = find_tag_spans(&self.buffer).into_iter().next() else {
                        continue;
                    };
                    self.buffer.drain(..tag.end);
                    match tag.kind {
                        TagKind::OpenReasoning => self.mode = LegacyReasoningMode::Reasoning,
                        TagKind::OpenFinal => {
                            self.saw_final = true;
                            self.mode = LegacyReasoningMode::Final;
                        }
                        _ => {}
                    }
                }
                LegacyReasoningMode::Reasoning => {
                    let Some(tag) = tags.iter().find(|tag| {
                        matches!(tag.kind, TagKind::CloseReasoning | TagKind::OpenFinal)
                    }) else {
                        let safe_len = safe_text_prefix_len(&self.buffer, finish);
                        if safe_len == 0 {
                            break;
                        }
                        let text = self.buffer.drain(..safe_len).collect::<String>();
                        if !text.is_empty() {
                            events.push(AgentEvent::ReasoningSummaryChunk(text));
                        }
                        continue;
                    };
                    if tag.start > 0 {
                        let text = self.buffer.drain(..tag.start).collect::<String>();
                        if !text.is_empty() {
                            events.push(AgentEvent::ReasoningSummaryChunk(text));
                        }
                    }
                    let Some(tag) = find_tag_spans(&self.buffer).into_iter().next() else {
                        continue;
                    };
                    self.buffer.drain(..tag.end);
                    match tag.kind {
                        TagKind::CloseReasoning => self.mode = LegacyReasoningMode::Text,
                        TagKind::OpenFinal => {
                            self.saw_final = true;
                            self.mode = LegacyReasoningMode::Final;
                        }
                        _ => {}
                    }
                }
                LegacyReasoningMode::Final => {
                    let Some(tag) = tags.iter().find(|tag| tag.kind == TagKind::CloseFinal) else {
                        let safe_len = safe_text_prefix_len(&self.buffer, finish);
                        if safe_len == 0 {
                            break;
                        }
                        let text = self.buffer.drain(..safe_len).collect::<String>();
                        if !text.is_empty() {
                            events.push(AgentEvent::TextChunk(text));
                        }
                        continue;
                    };
                    if tag.start > 0 {
                        let text = self.buffer.drain(..tag.start).collect::<String>();
                        if !text.is_empty() {
                            events.push(AgentEvent::TextChunk(text));
                        }
                    }
                    let Some(tag) = find_tag_spans(&self.buffer).into_iter().next() else {
                        continue;
                    };
                    self.buffer.drain(..tag.end);
                    self.mode = LegacyReasoningMode::Text;
                }
            }
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagKind {
    OpenReasoning,
    CloseReasoning,
    OpenFinal,
    CloseFinal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TagSpan {
    start: usize,
    end: usize,
    kind: TagKind,
}

fn find_tag_spans(input: &str) -> Vec<TagSpan> {
    let mut spans = Vec::new();
    let mut search_from = 0;
    while let Some(offset) = input[search_from..].find('<') {
        let start = search_from + offset;
        let Some(close_offset) = input[start..].find('>') else {
            break;
        };
        let end = start + close_offset + 1;
        if !is_inside_markdown_code(input, start)
            && let Some(kind) = classify_legacy_tag(&input[start..end])
        {
            spans.push(TagSpan { start, end, kind });
        }
        search_from = end;
    }
    spans
}

fn classify_legacy_tag(raw: &str) -> Option<TagKind> {
    let inner = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_ascii_lowercase();
    let closing = inner.starts_with('/');
    let name = inner
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default();
    match (closing, name) {
        (false, "think" | "thinking" | "reasoning") => Some(TagKind::OpenReasoning),
        (true, "think" | "thinking" | "reasoning") => Some(TagKind::CloseReasoning),
        (false, "final") => Some(TagKind::OpenFinal),
        (true, "final") => Some(TagKind::CloseFinal),
        _ => None,
    }
}

fn safe_text_prefix_len(input: &str, finish: bool) -> usize {
    if finish {
        return input.len();
    }
    match input.find('<') {
        Some(index) if !is_inside_markdown_code(input, index) => index,
        _ => input.len(),
    }
}

fn is_inside_markdown_code(input: &str, position: usize) -> bool {
    let mut in_fence = false;
    let mut line_start = true;
    let mut i = 0;
    while i < position {
        if line_start && input[i..].starts_with("```") {
            in_fence = !in_fence;
            line_start = false;
            i += 3;
            continue;
        }
        let Some(ch) = input[i..].chars().next() else {
            break;
        };
        line_start = ch == '\n';
        i += ch.len_utf8();
    }
    in_fence
}

/// Build a [`ToolCompletionRequest`] from the agent's reasoning context.
///
/// Extracted out of [`AgentResponder::respond`] so the streaming
/// variant produces a byte-identical request — keeping the two paths in
/// sync without duplicating the message-sanitisation rules.
fn build_request(ctx: &ReasoningContext) -> ToolCompletionRequest {
    let mut messages = ctx.messages.clone();
    sanitize_tool_messages(&mut messages);

    let mut request = ToolCompletionRequest::new(messages, ctx.available_tools.clone());
    if let Some(model) = ctx.model_override.as_ref() {
        request = request.with_model(model);
    }
    if !ctx.metadata.is_empty() {
        request.metadata = ctx.metadata.clone();
    }
    request
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

fn clean_respond_output(mut output: RespondOutput) -> RespondOutput {
    match &mut output.result {
        RespondResult::Text(text) => {
            *text = clean_user_visible_response(text);
        }
        RespondResult::ToolCalls { content, .. } => {
            if let Some(text) = content {
                *text = clean_user_visible_response(text);
            }
        }
    }
    output
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
            reasoning: None,
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
            reasoning: None,
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

    fn text_response_with_reasoning(text: &str, reasoning: &str) -> ToolCompletionResponse {
        ToolCompletionResponse {
            content: Some(text.to_string()),
            reasoning: Some(reasoning.to_string()),
            tool_calls: Vec::new(),
            input_tokens: 10,
            output_tokens: 20,
            finish_reason: FinishReason::Stop,
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

    #[tokio::test]
    async fn adapter_streaming_routes_native_reasoning_outside_text() {
        let provider = Arc::new(MockProvider::new(text_response_with_reasoning(
            "final answer",
            "private scratch",
        )));
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hi"));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::AgentEvent>(16);
        let output = responder.respond_streaming(&mut ctx, tx).await.unwrap();

        let mut text = String::new();
        let mut reasoning = String::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                crate::AgentEvent::TextChunk(delta) => text.push_str(&delta),
                crate::AgentEvent::ReasoningSummaryChunk(delta) => reasoning.push_str(&delta),
                other => panic!("unexpected event from adapter: {other:?}"),
            }
        }

        assert_eq!(reasoning, "private scratch");
        assert_eq!(text, "final answer");
        assert!(!text.contains("<think>"));
        match output.result {
            RespondResult::Text(t) => assert_eq!(t, "final answer"),
            _ => panic!("expected Text"),
        }
    }

    /// Provider that overrides the streaming entry point with a real
    /// chunked emission so we can verify the adapter forwards every
    /// chunk on `event_tx` and still returns the full `RespondOutput`.
    struct StreamingMockProvider {
        chunks: Vec<String>,
    }

    #[async_trait]
    impl LlmProvider for StreamingMockProvider {
        fn model_name(&self) -> &str {
            "mock-stream"
        }

        fn cost_per_token(&self) -> (Decimal, Decimal) {
            (Decimal::ZERO, Decimal::ZERO)
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            unreachable!()
        }

        async fn complete_with_tools(
            &self,
            _request: ToolCompletionRequest,
        ) -> Result<ToolCompletionResponse, LlmError> {
            unreachable!("streaming path should be exercised")
        }

        fn supports_streaming(&self) -> bool {
            true
        }

        async fn complete_with_tools_stream(
            &self,
            _request: ToolCompletionRequest,
            chunk_tx: tokio::sync::mpsc::UnboundedSender<String>,
        ) -> Result<ToolCompletionResponse, LlmError> {
            for chunk in &self.chunks {
                // A closed receiver shouldn't block real providers either.
                let _ = chunk_tx.send(chunk.clone());
            }
            Ok(text_response(&self.chunks.concat()))
        }
    }

    #[tokio::test]
    async fn req_dasclaw_runtime_agent_b2_adapter_respond_streaming_forwards_chunks() {
        let provider = Arc::new(StreamingMockProvider {
            chunks: vec!["foo ".into(), "bar ".into(), "baz".into()],
        });
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hi"));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::AgentEvent>(16);
        let output = responder.respond_streaming(&mut ctx, tx).await.unwrap();

        let mut got = Vec::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                crate::AgentEvent::TextChunk(s) => got.push(s),
                other => panic!("unexpected event from adapter: {other:?}"),
            }
        }
        assert_eq!(got, vec!["foo ", "bar ", "baz"]);

        match output.result {
            RespondResult::Text(t) => assert_eq!(t, "foo bar baz"),
            _ => panic!("expected Text"),
        }
    }

    #[tokio::test]
    async fn adapter_streaming_routes_legacy_think_tags_to_reasoning_events() {
        let provider = Arc::new(StreamingMockProvider {
            chunks: vec![
                "<thi".into(),
                "nk>private ".into(),
                "scratch</thi".into(),
                "nk>final ".into(),
                "answer".into(),
            ],
        });
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("hi"));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::AgentEvent>(16);
        let output = responder.respond_streaming(&mut ctx, tx).await.unwrap();

        let mut text = String::new();
        let mut reasoning = String::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                crate::AgentEvent::TextChunk(delta) => text.push_str(&delta),
                crate::AgentEvent::ReasoningSummaryChunk(delta) => reasoning.push_str(&delta),
                other => panic!("unexpected event from adapter: {other:?}"),
            }
        }

        assert_eq!(reasoning, "private scratch");
        assert_eq!(text, "final answer");
        match output.result {
            RespondResult::Text(t) => assert_eq!(t, "final answer"),
            _ => panic!("expected Text"),
        }
    }

    #[tokio::test]
    async fn adapter_streaming_keeps_think_tags_inside_code_fences_as_text() {
        let provider = Arc::new(StreamingMockProvider {
            chunks: vec![
                "```text\n<thi".into(),
                "nk>literal</think>\n```\n".into(),
                "final".into(),
            ],
        });
        let responder = LlmProviderResponder::new(Arc::clone(&provider));
        let mut ctx = ReasoningContext::new();
        ctx.messages.push(ChatMessage::user("show code"));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::AgentEvent>(16);
        let output = responder.respond_streaming(&mut ctx, tx).await.unwrap();

        let mut text = String::new();
        let mut reasoning = Vec::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                crate::AgentEvent::TextChunk(delta) => text.push_str(&delta),
                crate::AgentEvent::ReasoningSummaryChunk(delta) => reasoning.push(delta),
                other => panic!("unexpected event from adapter: {other:?}"),
            }
        }

        assert!(reasoning.is_empty(), "code fence must not become reasoning");
        assert_eq!(text, "```text\n<think>literal</think>\n```\nfinal");
        match output.result {
            RespondResult::Text(t) => {
                assert_eq!(t, "```text\n<think>literal</think>\n```\nfinal");
            }
            _ => panic!("expected Text"),
        }
    }
}
