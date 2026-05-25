//! Integration test for [`dasclaw_cli::run_with_tools`].
//!
//! ADR-153 §4.4 step 6 proof: a `dasclaw_runtime::Agent` constructed
//! from `build_responder` + `tools::default_builtins` runs a full
//! two-turn tool dispatch loop against a wiremock OpenAI-Chat-Completions
//! server. Turn 1 the model emits a `tool_call`, the local
//! [`StaticToolExecutor`](dasclaw_cli::tools::StaticToolExecutor) runs
//! the `echo` builtin, and turn 2 the model returns the final text.

use dasclaw_cli::provider::{ProviderArgs, ProviderKind, build_responder};
use dasclaw_cli::run_with_tools;
use dasclaw_cli::tools::default_builtins;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_KEY: &str = "sk-test";

fn tool_call_body() -> serde_json::Value {
    json!({
        "id": "chatcmpl-tool-call",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_echo_1",
                    "type": "function",
                    "function": {
                        "name": "echo",
                        "arguments": "{\"message\":\"hello-from-tool\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 8 }
    })
}

fn final_text_body() -> serde_json::Value {
    json!({
        "id": "chatcmpl-final",
        "object": "chat.completion",
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "tool said: hello-from-tool"
            },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 12, "completion_tokens": 6 }
    })
}

#[tokio::test]
async fn req_dasclaw_cli_tools_e2e_echo_tool_round_trips_through_agent() {
    let server = MockServer::start().await;

    // Turn 1: the assistant emits a tool_call. `up_to_n_times(1)` ensures
    // wiremock falls through to the second mock on the next request.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(tool_call_body()))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    // Turn 2: after the tool_result is fed back, the assistant finalizes.
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(final_text_body()))
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
    let executor = default_builtins();
    let definitions = executor.definitions();

    let reply = run_with_tools(
        responder,
        executor,
        definitions,
        "You are a test agent.",
        "please call echo",
    )
    .await
    .expect("agent run with tools succeeds");

    assert_eq!(reply.trim(), "tool said: hello-from-tool");
}
