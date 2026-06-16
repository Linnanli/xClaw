//! End-to-end tests for issue #908 / GUI blocker B2 on the
//! [`Session::stream`] facade.
//!
//! Coverage:
//!
//! 1. `Session::stream` forwards every [`AgentEvent::TextChunk`]
//!    the responder emits, in order, followed by exactly one
//!    [`AgentEvent::FinishReason`], and returns the concatenated text
//!    as the final reply.
//! 2. The streaming turn is recorded in the session history exactly
//!    like a non-streaming turn, so a follow-up `Session::invoke` sees
//!    the prior assistant message.

use std::sync::Arc;

use async_trait::async_trait;
use dasclaw_core::messages::FinishReason;
use dasclaw_core::reasoning_ctx::ReasoningContext;
use dasclaw_core::response_types::{RespondOutput, RespondResult, ResponseMetadata, TokenUsage};
use dasclaw_core::traits::HostError;
use dasclaw_runtime::{Agent, AgentEvent, AgentResponder, AgentRunOptions};
use dasclaw_session::Session;
use futures_util::StreamExt;
use tokio::sync::mpsc;

/// Responder that emits a fixed list of chunks on the streaming path
/// and the concatenation on the non-streaming path.
struct ChunkedResponder {
    chunks: Vec<&'static str>,
}

#[async_trait]
impl AgentResponder for ChunkedResponder {
    async fn respond(&self, _ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        Ok(RespondOutput {
            result: RespondResult::Text(self.chunks.concat()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }

    async fn respond_streaming(
        &self,
        _ctx: &mut ReasoningContext,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Result<RespondOutput, HostError> {
        for chunk in &self.chunks {
            event_tx
                .send(AgentEvent::TextChunk((*chunk).to_string()))
                .await
                .expect("event_rx alive");
        }
        Ok(RespondOutput {
            result: RespondResult::Text(self.chunks.concat()),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            metadata: ResponseMetadata::default(),
        })
    }
}

#[tokio::test]
async fn req_dasclaw_session_b2_stream_orders_chunks_and_finish() {
    let agent = Arc::new(
        Agent::builder()
            .responder(ChunkedResponder {
                chunks: vec!["He", "llo", ", ", "wor", "ld"],
            })
            .build()
            .expect("build agent"),
    );

    let mut session = Session::new(agent);
    let mut stream = session.stream("greet me", AgentRunOptions::stream());
    let mut events = Vec::new();
    let mut reply = None;
    while let Some(ev) = stream.next().await {
        match ev.expect("stream event") {
            AgentEvent::Completed(output) => {
                reply = Some(output.text);
            }
            ev => events.push(ev),
        }
    }

    let chunks: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextChunk(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(chunks, vec!["He", "llo", ", ", "wor", "ld"]);
    assert_eq!(events.len(), 6, "5 chunks + 1 finish_reason: {events:?}");
    assert!(
        matches!(
            events.last(),
            Some(AgentEvent::FinishReason(FinishReason::Stop))
        ),
        "last event must be FinishReason::Stop: {events:?}"
    );
    assert_eq!(reply.as_deref(), Some("Hello, world"));
}

#[tokio::test]
async fn req_dasclaw_session_b2_stream_records_assistant_turn_in_history() {
    // After a streaming turn, a follow-up non-streaming `invoke` must see
    // the prior user prompt + assistant reply in the conversation
    // history — same shape as two back-to-back `run` calls.
    let agent = Arc::new(
        Agent::builder()
            .responder(ChunkedResponder {
                chunks: vec!["hi", " there"],
            })
            .build()
            .expect("build agent"),
    );

    let mut session = Session::new(agent);
    let mut stream = session.stream("hello", AgentRunOptions::stream());
    let mut first = None;
    while let Some(event) = stream.next().await {
        if let AgentEvent::Completed(output) = event.expect("stream event") {
            first = Some(output.text);
        }
    }
    assert_eq!(first.as_deref(), Some("hi there"));

    // Session history must now hold the user prompt + the streamed
    // assistant reply. `Session::messages()` is a read-only view of the
    // accumulated history.
    let history_after_first = session.messages().len();
    assert_eq!(
        history_after_first, 2,
        "expected 1 user + 1 assistant message after streaming turn"
    );

    let second = session
        .invoke("what did you say?", AgentRunOptions::invoke())
        .await
        .expect("follow-up");
    assert_eq!(second.text, "hi there"); // ChunkedResponder is deterministic.
    assert!(
        session.messages().len() >= history_after_first + 2,
        "follow-up turn must extend history: have {} messages",
        session.messages().len()
    );
}
