//! `OpenAiCompatClient` 集成测试 — 走 wiremock 模拟 `/v1/chat/completions`，
//! 覆盖 happy path / 鉴权失败 / 限流退避 / 服务端错误重试 / 重试耗尽 /
//! 400 错误信封解析 / inline error envelope 兜底。

use std::time::Duration;

use dasclaw_llm_provider::{
    ApiError, InputMessage, MessageRequest, OpenAiCompatClient, OpenAiCompatConfig, RetryPolicy,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_KEY: &str = "sk-test";

fn sample_request() -> MessageRequest {
    MessageRequest {
        model: "gpt-4o-mini".to_string(),
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
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "pong"
            },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 1 }
    })
}

fn build_client(server_uri: String, retry: RetryPolicy) -> Result<OpenAiCompatClient, ApiError> {
    Ok(
        OpenAiCompatClient::new(TEST_API_KEY, OpenAiCompatConfig::openai())?
            .with_base_url(server_uri)
            .with_retry_policy(retry),
    )
}

#[tokio::test]
async fn complete_happy_path_propagates_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("request-id", "req-happy")
                .set_body_json(success_body()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(0)).expect("client constructible");
    let response = client
        .complete(&sample_request())
        .await
        .expect("request succeeds");
    assert_eq!(response.id, "chatcmpl-test");
    assert_eq!(response.request_id.as_deref(), Some("req-happy"));
    assert_eq!(response.stop_reason.as_deref(), Some("end_turn"));
}

#[tokio::test]
async fn auth_failure_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "message": "invalid api key" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(5)).expect("client constructible");
    let error = client
        .complete(&sample_request())
        .await
        .expect_err("auth fails");
    match error {
        ApiError::AuthFailed { reason, .. } => {
            assert!(reason.contains("invalid api key"), "{reason}");
        }
        other => panic!("expected AuthFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn rate_limited_then_success_honors_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "0")
                .set_body_json(json!({ "error": { "message": "slow down" } })),
        )
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(2)).expect("client constructible");
    let response = client
        .complete(&sample_request())
        .await
        .expect("eventually succeeds");
    assert_eq!(response.id, "chatcmpl-test");
}

#[tokio::test]
async fn server_error_retries_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(3)).expect("client constructible");
    let response = client
        .complete(&sample_request())
        .await
        .expect("eventually succeeds");
    assert_eq!(response.id, "chatcmpl-test");
}

#[tokio::test]
async fn max_retries_exhausted_returns_last_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500))
        .expect(3) // initial + 2 retries
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(2)).expect("client constructible");
    let error = client
        .complete(&sample_request())
        .await
        .expect_err("retries exhausted");
    assert!(matches!(error, ApiError::ServerError { status: 500, .. }));
}

#[tokio::test]
async fn bad_request_extracts_openai_error_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "unknown parameter foo" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(3)).expect("client constructible");
    let error = client
        .complete(&sample_request())
        .await
        .expect_err("bad request");
    match error {
        ApiError::BadRequest { reason, .. } => {
            assert!(reason.contains("unknown parameter foo"), "{reason}");
        }
        other => panic!("expected BadRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn inline_error_envelope_in_200_body_is_surfaced() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": { "message": "insufficient quota", "type": "billing" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = build_client(server.uri(), fast_retry_policy(0)).expect("client constructible");
    let error = client
        .complete(&sample_request())
        .await
        .expect_err("inline error surfaces");
    match error {
        ApiError::Provider { reason, .. } => {
            assert!(reason.contains("insufficient quota"), "{reason}");
        }
        other => panic!("expected Provider, got {other:?}"),
    }
}
