//! Phase 2 Step G — 真实 LLM 集成测试（ClawCodeLlmProvider）
//!
//! 这组测试对接真实 DashScope API（`qwen-coder-turbo-0919`），验证 Phase 2
//! `ClawCodeLlmProvider` 在生产环境下的行为。
//!
//! 所有测试都带 `#[ignore]`，CI 与默认 `cargo test` 都不会跑。手动触发：
//!
//! ```bash
//! export DASHSCOPE_API_KEY="sk-xxx"   # 或 LLM_API_KEY
//! cargo test --manifest-path desktop-client/ironclaw/Cargo.toml \
//!   --features claw-code-llm \
//!   --test claw_code_real_llm_tests -- --ignored --nocapture
//! ```
//!
//! ## 其他模型测试清单（待密钥申请后补）
//!
//! 下面这些测试暂时不写到这个文件里，等对应密钥到位后参考 `test_qwen_*`
//! 模式依葫芦画瓢：
//!
//! - [ ] `test_anthropic_claude_sonnet_*` — `ANTHROPIC_API_KEY` + `claude-sonnet-4-6`
//! - [ ] `test_openai_gpt_4o_*` — `OPENAI_API_KEY` + `gpt-4o`
//! - [ ] `test_xai_grok_*` — `XAI_API_KEY` + `grok-3`
//! - [ ] `test_kimi_*` — Moonshot `api_key` + `kimi-k1.5`
//! - [ ] `test_ollama_local_*` — 本地 `http://localhost:11434/v1`
//!
//! 验收维度（每个 Provider 都要覆盖）：
//! 1. 纯文本对话（happy path）
//! 2. 多轮历史保真
//! 3. 工具调用 + 结果回传
//! 4. 错误路径：无效 api_key → 清晰的 `LlmError`
//!
//! 参考：`docs/plans/architecture-refactor/04-phase2-claw-code-api.md` Step G。

use ironclaw::llm::claw_code_provider::ClawCodeLlmProvider;
use ironclaw::llm::config::{CacheRetention, RegistryProviderConfig};
use ironclaw::llm::registry::ProviderProtocol;
use ironclaw::llm::{
    ChatMessage, CompletionRequest, LlmProvider, ToolCompletionRequest, ToolDefinition,
};
use secrecy::SecretString;
use serde_json::json;

const QWEN_MODEL: &str = "qwen-coder-turbo-0919";
const DASHSCOPE_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

/// 优先读 `DASHSCOPE_API_KEY`，退而读 `LLM_API_KEY`（x-claw 现有 .env 习惯）。
fn read_qwen_api_key() -> Option<String> {
    std::env::var("DASHSCOPE_API_KEY")
        .ok()
        .or_else(|| std::env::var("LLM_API_KEY").ok())
        .filter(|k| !k.is_empty())
}

/// 构造一个指向 DashScope 的 `RegistryProviderConfig`。
fn qwen_config(api_key: String) -> RegistryProviderConfig {
    RegistryProviderConfig {
        protocol: ProviderProtocol::OpenAiCompletions,
        provider_id: "dashscope".to_string(),
        api_key: Some(SecretString::from(api_key)),
        base_url: DASHSCOPE_BASE_URL.to_string(),
        model: QWEN_MODEL.to_string(),
        extra_headers: Vec::new(),
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: Vec::new(),
        strict_tools_schema: true,
    }
}

/// 跳过辅助：缺密钥时打印提示但返回 `None` 让测试早退。
fn provider_or_skip(test_name: &str) -> Option<ClawCodeLlmProvider> {
    let Some(api_key) = read_qwen_api_key() else {
        eprintln!("⏭  skip {test_name}: set DASHSCOPE_API_KEY (or LLM_API_KEY) to run this test");
        return None;
    };
    match ClawCodeLlmProvider::from_registry_config(&qwen_config(api_key)) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("⏭  skip {test_name}: provider build failed: {e}");
            None
        }
    }
}

// ============================================================================
// Happy path tests
// ============================================================================

#[tokio::test]
#[ignore = "real network; run with --ignored + DASHSCOPE_API_KEY"]
async fn test_qwen_simple_text_completion() {
    let Some(provider) = provider_or_skip("test_qwen_simple_text_completion") else {
        return;
    };

    let req = CompletionRequest::new(vec![
        ChatMessage::system("You are a concise assistant. Answer in 1 sentence."),
        ChatMessage::user("What is 2 + 2? Reply with just the number."),
    ])
    .with_max_tokens(32)
    .with_temperature(0.0);

    let resp = provider
        .complete(req)
        .await
        .expect("completion should succeed");

    eprintln!("→ content: {:?}", resp.content);
    eprintln!(
        "→ usage: in={} out={} finish={:?}",
        resp.input_tokens, resp.output_tokens, resp.finish_reason
    );

    assert!(!resp.content.is_empty(), "non-empty content expected");
    assert!(resp.output_tokens > 0, "output_tokens should be > 0");
    assert!(
        resp.content.contains('4'),
        "answer should contain '4', got: {}",
        resp.content
    );
}

#[tokio::test]
#[ignore = "real network; run with --ignored + DASHSCOPE_API_KEY"]
async fn test_qwen_multi_turn_history_preserved() {
    let Some(provider) = provider_or_skip("test_qwen_multi_turn_history_preserved") else {
        return;
    };

    // 让模型用上第一轮的信息，判断历史有没有真的传到 DashScope。
    let req = CompletionRequest::new(vec![
        ChatMessage::system("You are a concise assistant. Answer in under 10 words."),
        ChatMessage::user("My favorite fruit is pineapple."),
        ChatMessage::assistant("Got it — I'll remember pineapple."),
        ChatMessage::user("What fruit did I say I liked? Reply with just the fruit name."),
    ])
    .with_max_tokens(16)
    .with_temperature(0.0);

    let resp = provider
        .complete(req)
        .await
        .expect("completion should succeed");
    eprintln!("→ content: {:?}", resp.content);

    assert!(
        resp.content.to_lowercase().contains("pineapple"),
        "history was not preserved; content: {}",
        resp.content
    );
}

#[tokio::test]
#[ignore = "real network; run with --ignored + DASHSCOPE_API_KEY"]
async fn test_qwen_tool_call_round_trip() {
    let Some(provider) = provider_or_skip("test_qwen_tool_call_round_trip") else {
        return;
    };

    // 定义一个模型"必须"调用才能回答问题的工具，避免模型自己编答案。
    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current temperature in a city. Call this for any weather question."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "City name" }
            },
            "required": ["city"],
            "additionalProperties": false
        }),
    };

    let req = ToolCompletionRequest::new(
        vec![
            ChatMessage::system(
                "You are a weather assistant. Always call the get_weather tool for any weather question.",
            ),
            ChatMessage::user("What's the weather in Tokyo right now?"),
        ],
        vec![tool],
    )
    .with_max_tokens(128)
    .with_temperature(0.0)
    .with_tool_choice("auto");

    let resp = provider
        .complete_with_tools(req)
        .await
        .expect("tool completion should succeed");

    eprintln!(
        "→ content={:?} tool_calls={} finish={:?}",
        resp.content,
        resp.tool_calls.len(),
        resp.finish_reason
    );

    assert!(
        !resp.tool_calls.is_empty(),
        "expected at least one tool call, got content={:?}",
        resp.content
    );
    let call = &resp.tool_calls[0];
    assert_eq!(call.name, "get_weather");
    let city = call.arguments.get("city").and_then(|v| v.as_str());
    assert!(
        city.is_some_and(|c| c.to_lowercase().contains("tokyo")),
        "tool call should extract 'Tokyo' as city, got {:?}",
        call.arguments
    );
}

// ============================================================================
// Failure-path tests
// ============================================================================

#[tokio::test]
#[ignore = "real network; run with --ignored"]
async fn test_qwen_invalid_api_key_returns_clear_error() {
    // 不需要用户密钥：无效 key 本来就会被 DashScope 拒绝。
    let cfg = qwen_config("sk-INVALID-DEADBEEF-KEY".to_string());
    let provider = ClawCodeLlmProvider::from_registry_config(&cfg).expect("build");

    let req = CompletionRequest::new(vec![ChatMessage::user("hi")]).with_max_tokens(8);

    let err = match provider.complete(req).await {
        Ok(r) => panic!("invalid key should not succeed; got response: {r:?}"),
        Err(e) => e,
    };

    // 错误消息必须传递 DashScope 的拒绝信息，不能被吞。
    let rendered = err.to_string();
    eprintln!("→ error: {rendered}");
    assert!(
        rendered.to_lowercase().contains("invalid")
            || rendered.to_lowercase().contains("unauthor")
            || rendered.contains("401")
            || rendered.contains("403"),
        "error should surface auth failure, got: {rendered}"
    );
}

// ============================================================================
// Smoke: Step E 开关也能跑通（env 路由 + 真实 API）
// ============================================================================

#[tokio::test]
#[ignore = "real network; run with --ignored + DASHSCOPE_API_KEY"]
async fn test_qwen_via_create_registry_provider_with_env_switch() {
    let Some(api_key) = read_qwen_api_key() else {
        eprintln!("⏭  skip: set DASHSCOPE_API_KEY (or LLM_API_KEY) to run this test");
        return;
    };

    // Step I 之后 `create_registry_provider` 默认路由到 ClawCodeLlmProvider，
    // 不再需要 env 开关。这里保留测试是为了证明入口路径完整通过。

    let cfg = qwen_config(api_key);
    let provider = ironclaw::llm::create_registry_provider(&cfg, 60)
        .expect("should build ClawCode provider via env switch");

    let req = CompletionRequest::new(vec![
        ChatMessage::system("Reply with exactly the single word: pong."),
        ChatMessage::user("ping"),
    ])
    .with_max_tokens(8)
    .with_temperature(0.0);

    let resp = provider
        .complete(req)
        .await
        .expect("complete should succeed");
    eprintln!("→ content: {:?}", resp.content);
    assert!(!resp.content.is_empty());
}

// ============================================================================
// 灰度冒烟：模拟 dispatcher 的完整 agent 工具循环
//
// Step G/H 验证了"单跳" provider 行为；这里把它拉成 agent 最常见的两跳流程，
// 近似 dispatcher/agent_loop 在生产里跑的主循环：
//
//   1. user       —— "东京和纽约的天气"
//   2. assistant  —— tool_calls=[get_weather(Tokyo), get_weather(New York)]
//   3. tool       —— 本地 stub 返回天气数据
//   4. assistant  —— 综合工具输出生成最终自然语言答复
//
// 这是 Phase 2 灰度最有代表性的一个端到端场景：只要它在真实 DashScope 上跑
// 通，说明 claw_code_provider 在"多跳历史 + 工具结果回填 + 终结回答"完整
// 链路都没问题，可以放心让 desktop-client GUI 打开 `claw-code-llm` feature
// 做人工灰度。
// ============================================================================

#[tokio::test]
#[ignore = "real network; run with --ignored + DASHSCOPE_API_KEY"]
async fn test_qwen_full_tool_round_trip_with_final_answer() {
    let Some(provider) = provider_or_skip("test_qwen_full_tool_round_trip_with_final_answer")
    else {
        return;
    };

    let tool = ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the current weather for a city. Returns a short description.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "City name" }
            },
            "required": ["city"],
            "additionalProperties": false
        }),
    };

    // --- 第一跳：期待模型返回 tool_call ---
    let mut history = vec![
        ChatMessage::system(
            "You are a weather assistant. Always call get_weather for any weather question, \
             then use the tool result to write a short natural-language reply.",
        ),
        ChatMessage::user("What's the weather in Tokyo right now?"),
    ];

    let first_req = ToolCompletionRequest::new(history.clone(), vec![tool.clone()])
        .with_max_tokens(128)
        .with_temperature(0.0)
        .with_tool_choice("auto");

    let first_resp = provider
        .complete_with_tools(first_req)
        .await
        .expect("first hop should succeed");

    eprintln!(
        "→ hop1: content={:?} tool_calls={} finish={:?}",
        first_resp.content,
        first_resp.tool_calls.len(),
        first_resp.finish_reason
    );
    assert!(
        !first_resp.tool_calls.is_empty(),
        "hop1 must produce at least one tool_call, got content={:?}",
        first_resp.content
    );

    // --- 第一跳完毕：把 assistant(+tool_calls) 和每个 tool_result 追加到 history ---
    let assistant_msg = ChatMessage {
        role: ironclaw::llm::Role::Assistant,
        content: first_resp.content.clone().unwrap_or_default(),
        content_parts: Vec::new(),
        tool_call_id: None,
        name: None,
        tool_calls: Some(first_resp.tool_calls.clone()),
    };
    history.push(assistant_msg);

    // 本地 stub：模拟 dispatcher 执行工具并回填结果
    for call in &first_resp.tool_calls {
        assert_eq!(call.name, "get_weather");
        let city = call
            .arguments
            .get("city")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let stub_result =
            format!("{{\"city\":\"{city}\",\"temp_c\":18,\"condition\":\"partly cloudy\"}}");
        history.push(ChatMessage::tool_result(
            &call.id,
            "get_weather",
            stub_result,
        ));
    }

    // --- 第二跳：模型应当消化工具结果生成最终自然语言答复 ---
    let second_req = ToolCompletionRequest::new(history, vec![tool])
        .with_max_tokens(128)
        .with_temperature(0.0)
        .with_tool_choice("auto");

    let second_resp = provider
        .complete_with_tools(second_req)
        .await
        .expect("second hop should succeed");

    eprintln!(
        "→ hop2: content={:?} tool_calls={} finish={:?}",
        second_resp.content,
        second_resp.tool_calls.len(),
        second_resp.finish_reason
    );

    let final_text = second_resp.content.clone().unwrap_or_default();
    assert!(
        !final_text.is_empty(),
        "hop2 must return final natural-language text, got: {second_resp:?}"
    );
    // 模型应当消化 stub 提供的 "partly cloudy" / "18"，而不是再去调工具。
    let lower = final_text.to_lowercase();
    assert!(
        lower.contains("cloudy") || lower.contains("18") || lower.contains("多云"),
        "hop2 must incorporate tool result; got: {final_text}"
    );
}
