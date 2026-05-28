//! `OpenAiCompatClient::stream` 集成测试 — 走 wiremock 模拟 `/v1/chat/completions`
//! 的 SSE 响应（`text/event-stream`，每帧 `data: {...}\n\n`，以 `data: [DONE]` 收尾），
//! 验证 `OpenAiCompatStream::next_event` 把 OpenAI `chat.completion.chunk` 协议正确
//! 翻译为 Anthropic 风格 [`StreamEvent`] 序列。

use dasclaw_llm_provider::{
    ApiError, ContentBlockDelta, InputMessage, MessageRequest, OpenAiCompatClient,
    OpenAiCompatConfig, OutputContentBlock, ProviderClient, RetryPolicy, StreamEvent,
};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_KEY: &str = "sk-test";
const SSE_CONTENT_TYPE: &str = "text/event-stream";

fn sample_request() -> MessageRequest {
    MessageRequest {
        model: "gpt-4o-mini".to_string(),
        max_tokens: 64,
        messages: vec![InputMessage::user_text("hello")],
        stream: true,
        ..MessageRequest::default()
    }
}

fn build_client(server_uri: String) -> Result<OpenAiCompatClient, ApiError> {
    Ok(
        OpenAiCompatClient::new(TEST_API_KEY, OpenAiCompatConfig::openai())?
            .with_base_url(server_uri)
            .with_retry_policy(RetryPolicy {
                max_retries: 0,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(2),
            }),
    )
}

/// 把多帧 `data: {json}\n\n` + `data: [DONE]\n\n` 拼成 SSE body。
fn sse_body(frames: &[&str]) -> String {
    let mut out = String::new();
    for frame in frames {
        out.push_str("data: ");
        out.push_str(frame);
        out.push_str("\n\n");
    }
    out.push_str("data: [DONE]\n\n");
    out
}

async fn drain_stream(
    client: &OpenAiCompatClient,
    request: &MessageRequest,
) -> Result<Vec<StreamEvent>, ApiError> {
    let mut stream = client.stream(request).await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next_event().await? {
        events.push(event);
    }
    Ok(events)
}

#[tokio::test]
async fn stream_text_chunks_yield_anthropic_event_sequence() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"id":"chatcmpl-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"He"},"finish_reason":null}]}"#,
        r#"{"id":"chatcmpl-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"content":"llo"},"finish_reason":null}]}"#,
        r#"{"id":"chatcmpl-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#,
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", SSE_CONTENT_TYPE)
                .insert_header("request-id", "req-stream-text")
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri()).expect("client constructible");
    let events = drain_stream(&client, &sample_request())
        .await
        .expect("stream drains");

    assert!(matches!(events.first(), Some(StreamEvent::MessageStart(_))));
    assert!(matches!(
        events.get(1),
        Some(StreamEvent::ContentBlockStart(_))
    ));

    let text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ContentBlockDelta(delta) => match &delta.delta {
                ContentBlockDelta::TextDelta { text } => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(text, "Hello");

    let last_two = &events[events.len() - 2..];
    assert!(matches!(last_two[0], StreamEvent::MessageDelta(_)));
    assert!(matches!(last_two[1], StreamEvent::MessageStop(_)));

    let stop_reason = match &last_two[0] {
        StreamEvent::MessageDelta(ev) => ev.delta.stop_reason.as_deref(),
        _ => None,
    };
    assert_eq!(stop_reason, Some("end_turn"));
}

#[tokio::test]
async fn stream_tool_call_accumulates_input_json_delta() {
    let server = MockServer::start().await;
    let body = sse_body(&[
        // 第一帧：tool call 元信息 + 第一段 arguments
        r#"{"id":"c-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_1","function":{"name":"get_weather","arguments":"{\"loc"}}]},"finish_reason":null}]}"#,
        // 第二帧：剩余 arguments
        r#"{"id":"c-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ation\":\"SH\"}"}}]},"finish_reason":null}]}"#,
        // 第三帧：finish
        r#"{"id":"c-1","model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":12,"completion_tokens":7}}"#,
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", SSE_CONTENT_TYPE)
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri()).expect("client constructible");
    let events = drain_stream(&client, &sample_request())
        .await
        .expect("stream drains");

    // ContentBlockStart for ToolUse with name + id
    let start = events
        .iter()
        .find_map(|e| match e {
            StreamEvent::ContentBlockStart(ev) => match &ev.content_block {
                OutputContentBlock::ToolUse { id, name, .. } => Some((id.clone(), name.clone())),
                _ => None,
            },
            _ => None,
        })
        .expect("tool_use start emitted");
    assert_eq!(start.0, "call_1");
    assert_eq!(start.1, "get_weather");

    // 拼接所有 InputJsonDelta，校验完整 arguments
    let arguments: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ContentBlockDelta(delta) => match &delta.delta {
                ContentBlockDelta::InputJsonDelta { partial_json } => Some(partial_json.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(arguments, r#"{"location":"SH"}"#);

    // ContentBlockStop with index = 1（block_index = openai_index + 1）
    let stop_idx = events.iter().rev().find_map(|e| match e {
        StreamEvent::ContentBlockStop(ev) => Some(ev.index),
        _ => None,
    });
    assert_eq!(stop_idx, Some(1));

    let stop_reason = events.iter().rev().find_map(|e| match e {
        StreamEvent::MessageDelta(ev) => ev.delta.stop_reason.clone(),
        _ => None,
    });
    assert_eq!(stop_reason.as_deref(), Some("tool_use"));
}

#[tokio::test]
async fn stream_tolerates_explicit_null_tool_calls() {
    // DashScope/Qwen 等部分后端会把 `tool_calls: null` 发出来，serde `default`
    // 单独处理不了显式 null，本测试守护 `deserialize_null_as_empty_vec` 行为。
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"id":"c-2","model":"qwen-max","choices":[{"index":0,"delta":{"role":"assistant","content":"ok","tool_calls":null},"finish_reason":null}]}"#,
        r#"{"id":"c-2","model":"qwen-max","choices":[{"index":0,"delta":{"content":null,"tool_calls":null},"finish_reason":"stop"}]}"#,
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", SSE_CONTENT_TYPE)
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri()).expect("client constructible");
    let events = drain_stream(&client, &sample_request())
        .await
        .expect("stream drains");
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ContentBlockDelta(delta) => match &delta.delta {
                ContentBlockDelta::TextDelta { text } => Some(text.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(text, "ok");
}

#[tokio::test]
async fn stream_dispatch_through_provider_client_returns_same_events() {
    // ProviderClient::stream_message 的 dispatcher 行为：OpenAi 变体应走
    // OpenAiCompatStream 路径，与直连一致。
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"id":"c-3","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"hi"},"finish_reason":null}]}"#,
        r#"{"id":"c-3","model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", SSE_CONTENT_TYPE)
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let inner = build_client(server.uri()).expect("client constructible");
    let provider = ProviderClient::OpenAi(inner);
    let mut stream = provider
        .stream_message(&sample_request())
        .await
        .expect("dispatch ok");

    let mut text = String::new();
    while let Some(event) = stream.next_event().await.expect("next ok") {
        if let StreamEvent::ContentBlockDelta(delta) = event
            && let ContentBlockDelta::TextDelta { text: chunk } = delta.delta
        {
            text.push_str(&chunk);
        }
    }
    assert_eq!(text, "hi");
}

#[tokio::test]
async fn stream_message_delta_carries_cache_read_input_tokens() {
    // 流式收尾的 `usage` 帧含 `prompt_tokens_details.cached_tokens` 时，必须
    // 透传到 MessageDelta 事件的 `usage.cache_read_input_tokens`，并按
    // Anthropic 语义把缓存 token 从 `input_tokens` 中扣除。
    let server = MockServer::start().await;
    let body = sse_body(&[
        r#"{"id":"c-cache","model":"gpt-4o-mini","choices":[{"index":0,"delta":{"role":"assistant","content":"ok"},"finish_reason":null}]}"#,
        r#"{"id":"c-cache","model":"gpt-4o-mini","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":75}}}"#,
    ]);
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", SSE_CONTENT_TYPE)
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri()).expect("client constructible");
    let events = drain_stream(&client, &sample_request())
        .await
        .expect("stream drains");
    let usage = events
        .iter()
        .rev()
        .find_map(|e| match e {
            StreamEvent::MessageDelta(ev) => Some(ev.usage.clone()),
            _ => None,
        })
        .expect("message_delta carries usage");
    assert_eq!(usage.input_tokens, 25);
    assert_eq!(usage.cache_read_input_tokens, 75);
    assert_eq!(usage.cache_creation_input_tokens, 0);
    assert_eq!(usage.output_tokens, 2);
}
