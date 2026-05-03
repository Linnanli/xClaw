//! 集成测试 — `AnthropicClient` 的 wire 行为。
//!
//! 用 [`wiremock`] mock 上游 `/v1/messages` 端点，验证：
//! 1. `complete()` happy path + `request-id` header propagation
//! 2. 401 不重试，立即返回 `ApiError::AuthFailed`
//! 3. 429 携带 `Retry-After` → 退避采纳上游建议，下一次成功
//! 4. 500 重试后成功（默认指数退避）
//! 5. 最大重试次数耗尽 → 冒泡最后一次错误
//! 6. 流式响应 SSE 帧可逐个拉取，结束后 `next_event` 返回 `Ok(None)`
//! 7. `400` 含 Anthropic 错误 envelope → `ApiError::BadRequest.reason` 为 `error.message`

use std::time::Duration;

use dasclaw_llm_provider::{
    AnthropicClient, ApiError, AuthSource, InputMessage, MessageRequest, RetryPolicy, StreamEvent,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_KEY: &str = "sk-ant-test";

fn sample_request() -> MessageRequest {
    MessageRequest {
        model: "claude-3-5-sonnet-20241022".to_string(),
        max_tokens: 16,
        messages: vec![InputMessage::user_text("ping")],
        ..MessageRequest::default()
    }
}

fn fast_retry_policy(max_retries: u32) -> RetryPolicy {
    RetryPolicy {
        max_retries,
        initial_backoff: Duration::from_millis(1),
        max_backoff: Duration::from_millis(5),
    }
}

fn success_body() -> serde_json::Value {
    json!({
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-5-sonnet-20241022",
        "content": [
            { "type": "text", "text": "pong" }
        ],
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 3,
            "output_tokens": 1
        }
    })
}

fn build_client(server_uri: String, retry: RetryPolicy) -> Result<AnthropicClient, ApiError> {
    Ok(
        AnthropicClient::with_auth(AuthSource::ApiKey(TEST_API_KEY.into()))?
            .with_base_url(server_uri)
            .with_retry_policy(retry),
    )
}

#[tokio::test]
async fn complete_happy_path_propagates_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", TEST_API_KEY))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("request-id", "req-success")
                .set_body_json(success_body()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(0)).expect("client constructible");
    let response = client.complete(&sample_request()).await.expect("ok");
    assert_eq!(response.request_id.as_deref(), Some("req-success"));
    assert_eq!(response.usage.input_tokens, 3);
}

#[tokio::test]
async fn auth_failure_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "type": "error",
            "error": { "type": "authentication_error", "message": "invalid x-api-key" }
        })))
        .expect(1) // exactly one — no retry
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(5)).expect("client constructible");
    let err = client
        .complete(&sample_request())
        .await
        .expect_err("should fail");
    match err {
        ApiError::AuthFailed { reason, .. } => {
            assert!(
                reason.contains("invalid x-api-key"),
                "reason should carry server message, got {reason}"
            );
        }
        other => panic!("expected AuthFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn rate_limited_then_success_honors_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "0")
                .insert_header("request-id", "req-rl")
                .set_body_json(json!({
                    "type": "error",
                    "error": { "type": "rate_limit_error", "message": "slow down" }
                })),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(2)).expect("client constructible");
    let response = client.complete(&sample_request()).await.expect("ok");
    assert_eq!(response.usage.output_tokens, 1);
}

#[tokio::test]
async fn server_error_retries_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream busy"))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(3)).expect("client constructible");
    let response = client.complete(&sample_request()).await.expect("ok");
    assert_eq!(response.usage.input_tokens, 3);
}

#[tokio::test]
async fn max_retries_exhausted_returns_last_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream busy"))
        .expect(3) // 1 initial + 2 retries
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(2)).expect("client constructible");
    let err = client
        .complete(&sample_request())
        .await
        .expect_err("should fail");
    match err {
        ApiError::ServerError { status, .. } => assert_eq!(status, 503),
        other => panic!("expected ServerError, got {other:?}"),
    }
}

#[tokio::test]
async fn bad_request_extracts_anthropic_error_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "error",
            "error": { "type": "invalid_request_error", "message": "max_tokens > 4096" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(3)).expect("client constructible");
    let err = client
        .complete(&sample_request())
        .await
        .expect_err("should fail");
    match err {
        ApiError::BadRequest { reason, .. } => assert_eq!(reason, "max_tokens > 4096"),
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn stream_yields_events_until_message_stop() {
    let body = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-3-5-sonnet-20241022\",\"content\":[],\"stop_reason\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"pong\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .insert_header("request-id", "req-stream")
                .set_body_string(body),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(0)).expect("client constructible");
    let mut stream = client.stream(&sample_request()).await.expect("ok");
    assert_eq!(stream.request_id(), Some("req-stream"));

    let mut events = Vec::new();
    while let Some(event) = stream.next_event().await.expect("event") {
        events.push(event);
    }
    assert_eq!(events.len(), 6, "expected 6 events, got {events:?}");
    assert!(matches!(events[0], StreamEvent::MessageStart(_)));
    assert!(matches!(events[5], StreamEvent::MessageStop(_)));
}
