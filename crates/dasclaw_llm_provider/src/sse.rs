//! SSE 帧解析器 — 把 `data: ...` 行流解析为 [`StreamEvent`]。
//!
//! 本解析器是同步、增量、零异步依赖的字节缓冲器，由上层（PR-A.2 / PR-A.3）的
//! `reqwest::Response` 字节流驱动。设计参考 Anthropic Messages SSE 协议
//! （`event: ` + `data: ` + 空行帧分隔），同时兼容 `: keepalive` 注释行
//! 与 `data: [DONE]` 终止标识。
//!
//! # 与 `claw-code-api::sse` 的差异
//!
//! 1. **JSON 错误映射**：本 crate 的 [`ApiError`] 没有专用的 `JsonDeserialize`
//!    变体；不可解析的 SSE 帧统一映射为 [`ApiError::MalformedSseFrame`]，错误
//!    消息内嵌 provider / model / payload 截断（≤ 200 字节，避免泄漏密钥）。
//! 2. **provider/model 上下文可选**：未携带上下文时占位为 `"unknown"`，
//!    上层可通过 [`SseParser::with_context`] 显式注入。
//!
//! [ApiError]: crate::error::ApiError
//! [StreamEvent]: crate::types::StreamEvent

use crate::error::ApiError;
use crate::types::StreamEvent;

/// SSE 流增量解析器。
///
/// 内部维护字节缓冲，按 `\n\n` 或 `\r\n\r\n` 切帧；不完整帧保留至下次 `push`。
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    provider: Option<String>,
    model: Option<String>,
}

impl SseParser {
    /// 构造无上下文的解析器（错误消息中 provider/model 显示为 `"unknown"`）。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入 provider 名与 model 名，仅用于增强 `MalformedSseFrame` 错误信息。
    #[must_use]
    pub fn with_context(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self.model = Some(model.into());
        self
    }

    /// 推送一段字节，返回本次能完整解析出的 [`StreamEvent`] 列表。
    ///
    /// 不完整帧自动保留至下次 `push`。`event: ping` 与 `data: [DONE]` 会被静默丢弃。
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<StreamEvent>, ApiError> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();

        while let Some(frame) = self.next_frame() {
            if let Some(event) = self.parse_frame_with_context(&frame)? {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// 流结束时调用 — 把缓冲区残留作为最后一帧解析。
    pub fn finish(&mut self) -> Result<Vec<StreamEvent>, ApiError> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        let trailing = std::mem::take(&mut self.buffer);
        match self.parse_frame_with_context(&String::from_utf8_lossy(&trailing))? {
            Some(event) => Ok(vec![event]),
            None => Ok(Vec::new()),
        }
    }

    fn parse_frame_with_context(&self, frame: &str) -> Result<Option<StreamEvent>, ApiError> {
        let provider = self.provider.as_deref().unwrap_or("unknown");
        let model = self.model.as_deref().unwrap_or("unknown");
        parse_frame_with_provider(frame, provider, model)
    }

    fn next_frame(&mut self) -> Option<String> {
        let separator = self
            .buffer
            .windows(2)
            .position(|window| window == b"\n\n")
            .map(|position| (position, 2))
            .or_else(|| {
                self.buffer
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|position| (position, 4))
            })?;

        let (position, separator_len) = separator;
        let frame = self
            .buffer
            .drain(..position + separator_len)
            .collect::<Vec<_>>();
        let frame_len = frame.len().saturating_sub(separator_len);
        Some(String::from_utf8_lossy(&frame[..frame_len]).into_owned())
    }
}

/// 解析单帧（不带 provider/model 上下文）；内部测试和无上下文调用方使用。
pub fn parse_frame(frame: &str) -> Result<Option<StreamEvent>, ApiError> {
    parse_frame_with_provider(frame, "unknown", "unknown")
}

fn parse_frame_with_provider(
    frame: &str,
    provider: &str,
    model: &str,
) -> Result<Option<StreamEvent>, ApiError> {
    let trimmed = frame.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut data_lines = Vec::new();
    let mut event_name: Option<&str> = None;

    for line in trimmed.lines() {
        if line.starts_with(':') {
            continue;
        }
        if let Some(name) = line.strip_prefix("event:") {
            event_name = Some(name.trim());
            continue;
        }
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }

    if matches!(event_name, Some("ping")) {
        return Ok(None);
    }

    if data_lines.is_empty() {
        return Ok(None);
    }

    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(None);
    }

    serde_json::from_str::<StreamEvent>(&payload)
        .map(Some)
        .map_err(|error| {
            let truncated = truncate_for_error(&payload);
            ApiError::MalformedSseFrame(format!(
                "{provider} model={model} payload={truncated}: {error}"
            ))
        })
}

/// 截断 payload 到 ≤ 200 字节，避免错误消息泄漏完整 prompt / 密钥片段。
fn truncate_for_error(payload: &str) -> String {
    const MAX: usize = 200;
    if payload.len() <= MAX {
        return payload.to_string();
    }
    let safe_end = (0..=MAX)
        .rev()
        .find(|&i| payload.is_char_boundary(i))
        .unwrap_or(0);
    format!(
        "{}…(truncated, {} bytes)",
        &payload[..safe_end],
        payload.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_frame, SseParser};
    use crate::types::{
        ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, MessageDelta,
        MessageDeltaEvent, MessageStopEvent, OutputContentBlock, StreamEvent, Usage,
    };

    #[test]
    fn parses_single_content_block_start_frame() {
        let frame = concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"Hi\"}}\n\n"
        );
        let event = parse_frame(frame).expect("frame should parse");
        assert_eq!(
            event,
            Some(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                index: 0,
                content_block: OutputContentBlock::Text {
                    text: "Hi".to_string(),
                },
            }))
        );
    }

    #[test]
    fn parses_chunked_stream_buffering_partial_frame() {
        let mut parser = SseParser::new();
        let first = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel";
        let second = b"lo\"}}\n\n";

        let early = parser.push(first).expect("first chunk should buffer");
        assert!(early.is_empty(), "partial frame must not yield events yet");
        let events = parser.push(second).expect("second chunk should parse");
        assert_eq!(
            events,
            vec![StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                index: 0,
                delta: ContentBlockDelta::TextDelta {
                    text: "Hello".to_string(),
                },
            })]
        );
    }

    #[test]
    fn ignores_keepalive_comments_ping_events_and_done_marker() {
        let mut parser = SseParser::new();
        let payload = concat!(
            ": keepalive\n",
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
            "data: [DONE]\n\n"
        );
        let events = parser
            .push(payload.as_bytes())
            .expect("parser should succeed");
        assert_eq!(
            events,
            vec![
                StreamEvent::MessageDelta(MessageDeltaEvent {
                    delta: MessageDelta {
                        stop_reason: Some("tool_use".to_string()),
                        stop_sequence: None,
                    },
                    usage: Usage {
                        input_tokens: 1,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                        output_tokens: 2,
                    },
                }),
                StreamEvent::MessageStop(MessageStopEvent {}),
            ]
        );
    }

    #[test]
    fn ignores_data_less_event_frames() {
        let frame = "event: ping\n\n";
        let event = parse_frame(frame).expect("frame should parse");
        assert!(event.is_none());
    }

    #[test]
    fn supports_crlf_separator() {
        let mut parser = SseParser::new();
        let payload = b"event: message_stop\r\ndata: {\"type\":\"message_stop\"}\r\n\r\n";
        let events = parser.push(payload).expect("crlf parser should succeed");
        assert_eq!(events, vec![StreamEvent::MessageStop(MessageStopEvent {})]);
    }

    #[test]
    fn malformed_json_returns_malformed_sse_frame_error_with_context() {
        let frame = "event: message_delta\ndata: {not valid json}\n\n";
        let err = parse_frame(frame).expect_err("invalid JSON must fail");
        match err {
            crate::error::ApiError::MalformedSseFrame(msg) => {
                assert!(msg.contains("unknown"), "expect provider context: {msg}");
                assert!(msg.contains("model="), "expect model context: {msg}");
            }
            other => panic!("expected MalformedSseFrame, got {other:?}"),
        }
    }

    #[test]
    fn parser_with_context_includes_provider_and_model_in_error() {
        let mut parser = SseParser::new().with_context("anthropic", "claude-sonnet-4-6");
        let frame = b"event: bogus\ndata: {not valid json}\n\n";
        let err = parser.push(frame).expect_err("invalid JSON must fail");
        match err {
            crate::error::ApiError::MalformedSseFrame(msg) => {
                assert!(
                    msg.contains("anthropic"),
                    "context provider missing in: {msg}"
                );
                assert!(
                    msg.contains("claude-sonnet-4-6"),
                    "context model missing in: {msg}"
                );
            }
            other => panic!("expected MalformedSseFrame, got {other:?}"),
        }
    }

    #[test]
    fn finish_drains_trailing_partial_frame_when_terminated_without_double_newline() {
        let mut parser = SseParser::new();
        // Frame missing the terminating \n\n: parser should buffer it.
        let payload = b"event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let early = parser.push(payload).expect("partial push should buffer");
        assert!(early.is_empty());
        let final_events = parser.finish().expect("finish should drain");
        assert_eq!(
            final_events,
            vec![StreamEvent::MessageStop(MessageStopEvent {})]
        );
    }

    #[test]
    fn finish_on_empty_buffer_yields_nothing() {
        let mut parser = SseParser::new();
        let events = parser.finish().expect("empty finish ok");
        assert!(events.is_empty());
    }

    #[test]
    fn empty_frame_returns_none() {
        let event = parse_frame("").expect("empty frame should parse");
        assert!(event.is_none());
    }

    #[test]
    fn truncated_payload_stays_within_size_bound() {
        // Build a long invalid payload to test the truncation path of error formatting.
        let big = format!("event: x\ndata: {{ {} }}\n\n", "x".repeat(500));
        let err = parse_frame(&big).expect_err("invalid JSON must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("truncated"),
            "should mention truncation: {msg}"
        );
    }

    #[test]
    fn multiple_data_lines_concatenate_with_newline() {
        // Anthropic MCP and some providers split JSON across multiple data: lines.
        let frame = concat!(
            "event: message_stop\n",
            "data: {\"type\":\n",
            "data: \"message_stop\"}\n\n"
        );
        let event = parse_frame(frame).expect("multiline data should parse");
        assert_eq!(event, Some(StreamEvent::MessageStop(MessageStopEvent {})));
    }
}
