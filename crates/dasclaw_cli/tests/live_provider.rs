//! Integration test for the real-LLM wiring in `dasclaw_cli::provider`.
//!
//! ADR-153 §4.4 step 5 proof: a `dasclaw_runtime::Agent` constructed
//! purely from `dasclaw_cli::provider::build_responder` drives a real
//! OpenAI-Chat-Completions wire format end-to-end against a wiremock
//! server. No desktop deps, no network, no API key — the mock answers
//! `"pong"` and the agent's reply round-trips through the entire stack.

use dasclaw_cli::provider::{ProviderArgs, ProviderKind, build_responder};
use dasclaw_cli::run;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_KEY: &str = "sk-test";

fn success_body(reply: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": reply },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 3, "completion_tokens": 1 }
    })
}

#[tokio::test]
async fn openai_compat_round_trips_through_agent() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("pong")))
        .expect(1)
        .mount(&server)
        .await;

    let args = ProviderArgs {
        provider: ProviderKind::OpenaiCompat,
        model: Some("gpt-4o-mini".to_string()),
        base_url: Some(server.uri()),
        api_key: Some(TEST_API_KEY.to_string()),
    };
    let responder = build_responder(&args).expect("responder builds");
    let reply = run(responder, "You are a test agent.", "ping")
        .await
        .expect("agent run succeeds");

    assert_eq!(reply.trim(), "pong");
}
