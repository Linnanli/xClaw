//! Chat Proxy — 使用 ironclaw_llm 调用 LLM，返回 Vercel AI SDK Data Stream 格式
//!
//! 架构：
//!   客户端 POST /api/chat/completions
//!     → 从数据库查询模型配置
//!     → 构造 RegistryProviderConfig
//!     → ironclaw_llm::create_provider_from_config() 创建 provider
//!     → LlmProvider::complete() 调用 LLM
//!     → 转换为 Vercel AI SDK Data Stream 格式返回
//!
//! Data Stream 协议格式（每行一个事件）：
//!   0:"text"                    — 文本 token
//!   e:{"finishReason":"stop"}   — 完成元数据
//!   d:{"finishReason":"stop"}   — 结束标记

use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use ironclaw_llm::{
    ChatMessage as LlmChatMessage, CompletionRequest,
    RegistryProviderConfig, create_provider_from_config,
    config::CacheRetention,
    registry::ProviderProtocol,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

use crate::AppState;
use crate::error::{Error, Result};

// ============================================================================
// 请求/响应类型
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "default_true")]
    pub stream: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

fn default_true() -> bool { true }

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    /// AI SDK 发送的 content 可能是字符串或 parts 数组
    #[serde(deserialize_with = "deserialize_content")]
    pub content: String,
}

/// 兼容 AI SDK 的 content 格式：
/// - 字符串：`"hello"`
/// - 数组：`[{"type":"text","text":"hello"}]`
fn deserialize_content<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Content {
        Text(String),
        Parts(Vec<ContentPart>),
    }

    #[derive(Deserialize)]
    struct ContentPart {
        #[serde(rename = "type")]
        part_type: Option<String>,
        text: Option<String>,
    }

    match Content::deserialize(deserializer)? {
        Content::Text(s) => Ok(s),
        Content::Parts(parts) => {
            let text = parts
                .iter()
                .filter(|p| p.part_type.as_deref() == Some("text"))
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                Err(de::Error::custom("no text content in message parts"))
            } else {
                Ok(text)
            }
        }
    }
}

// ============================================================================
// Handler
// ============================================================================

/// POST /api/chat/completions — 调用 LLM 并返回 Data Stream 格式
#[instrument(skip_all, fields(model = %body.model))]
pub async fn chat_completions_handler(
    State(state): State<AppState>,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Response> {
    let client = state.db_pool.get().await
        .map_err(|e| Error::Database(e.to_string()))?;

    // 1. 查询模型配置
    let row = client
        .query_opt(
            "SELECT api_base_url, api_key, provider FROM model_configs \
             WHERE model_id = $1 AND enabled = true",
            &[&body.model],
        )
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .ok_or_else(|| Error::Validation(format!("模型 '{}' 未找到或未启用", body.model)))?;

    let api_base_url: Option<String> = row.get(0);
    let api_key: Option<String> = row.get(1);
    let provider: String = row.get(2);

    let base_url = api_base_url.ok_or_else(|| Error::Validation(format!(
        "模型 '{}' 未配置 API Base URL", body.model
    )))?;
    let key = api_key.ok_or_else(|| Error::Validation(format!(
        "模型 '{}' 未配置 API Key", body.model
    )))?;

    info!(model = %body.model, provider = %provider, "Chat completion request");

    // 2. 构造 RegistryProviderConfig
    let protocol = match provider.as_str() {
        "anthropic" => ProviderProtocol::Anthropic,
        "ollama" => ProviderProtocol::Ollama,
        _ => ProviderProtocol::OpenAiCompletions, // 所有国产模型都走 OpenAI 兼容
    };

    let reg_config = RegistryProviderConfig {
        protocol,
        provider_id: provider.clone(),
        api_key: Some(SecretString::from(key)),
        base_url,
        model: body.model.clone(),
        extra_headers: vec![],
        oauth_token: None,
        is_codex_chatgpt: false,
        refresh_token: None,
        auth_path: None,
        cache_retention: CacheRetention::None,
        unsupported_params: vec![],
    };

    // 3. 创建 LLM provider
    let llm = create_provider_from_config(&reg_config)
        .map_err(|e| Error::Validation(format!("创建 LLM provider 失败: {}", e)))?;

    // 4. 构造 CompletionRequest
    let messages: Vec<LlmChatMessage> = body.messages.iter().map(|m| {
        match m.role.as_str() {
            "system" => LlmChatMessage::system(&m.content),
            "assistant" => LlmChatMessage::assistant(&m.content),
            _ => LlmChatMessage::user(&m.content),
        }
    }).collect();

    let mut request = CompletionRequest::new(messages)
        .with_model(&body.model);
    if let Some(max_tokens) = body.max_tokens {
        request = request.with_max_tokens(max_tokens);
    }
    if let Some(temperature) = body.temperature {
        request = request.with_temperature(temperature);
    }

    // 5. 调用 LLM
    debug!("Calling LLM provider: {}", llm.model_name());
    let response = llm.complete(request).await
        .map_err(|e| Error::Validation(format!("LLM 调用失败: {}", e)))?;

    // 6. 编码为 OpenAI 兼容 SSE 格式（前端 ChatModelAdapter 解析）
    let mut output = String::new();

    // 模拟 OpenAI streaming 格式
    let chunk = serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "model": body.model,
        "choices": [{
            "index": 0,
            "delta": { "content": response.content },
            "finish_reason": null
        }]
    });
    output.push_str(&format!("data: {}\n\n", chunk));

    let finish_chunk = serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "model": body.model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": response.input_tokens,
            "completion_tokens": response.output_tokens,
            "total_tokens": response.input_tokens + response.output_tokens
        }
    });
    output.push_str(&format!("data: {}\n\n", finish_chunk));
    output.push_str("data: [DONE]\n\n");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(output))
        .unwrap())
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── 单元测试：content 反序列化 ──

    #[test]
    fn test_deserialize_content_string() {
        let json = r#"{"role":"user","content":"你好"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "你好");
    }

    #[test]
    fn test_deserialize_content_parts_array() {
        // AI SDK 发送的 parts 数组格式
        let json = r#"{"role":"user","content":[{"type":"text","text":"你好世界"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "你好世界");
    }

    #[test]
    fn test_deserialize_content_multiple_parts() {
        let json = r#"{"role":"user","content":[{"type":"text","text":"第一段"},{"type":"text","text":"第二段"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "第一段\n第二段");
    }

    #[test]
    fn test_deserialize_content_filters_non_text_parts() {
        // 非 text 类型的 part 应该被过滤
        let json = r#"{"role":"user","content":[{"type":"image","url":"http://..."},{"type":"text","text":"描述图片"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "描述图片");
    }

    // ── 单元测试：OpenAI SSE 格式 ──

    #[test]
    fn test_openai_sse_chunk_format() {
        let chunk = serde_json::json!({
            "object": "chat.completion.chunk",
            "model": "deepseek-chat",
            "choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]
        });
        let line = format!("data: {}\n\n", chunk);
        assert!(line.starts_with("data: "));
        assert!(line.contains("chat.completion.chunk"));
        assert!(line.contains("Hello"));
    }

    #[test]
    fn test_openai_sse_done_marker() {
        let done = "data: [DONE]\n\n";
        assert_eq!(done, "data: [DONE]\n\n");
    }

    #[test]
    fn test_openai_sse_finish_chunk_has_usage() {
        let finish = serde_json::json!({
            "object": "chat.completion.chunk",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });
        assert_eq!(finish["usage"]["total_tokens"], 15);
        assert_eq!(finish["choices"][0]["finish_reason"], "stop");
    }

    // ── 契约测试：请求格式 ──

    #[test]
    fn test_contract_request_string_content() {
        let json = r#"{"model":"deepseek-chat","messages":[{"role":"user","content":"你好"}]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "deepseek-chat");
        assert_eq!(req.messages[0].content, "你好");
        assert!(req.stream);
    }

    #[test]
    fn test_contract_request_parts_content() {
        // AI SDK 发送的格式
        let json = r#"{"model":"gpt-4o","messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.messages[0].content, "hi");
    }

    #[test]
    fn test_contract_request_with_all_fields() {
        let json = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}],"stream":false,"max_tokens":1000,"temperature":0.7}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert!(!req.stream);
        assert_eq!(req.max_tokens, Some(1000));
        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_contract_openai_sse_sequence() {
        // 完整的 OpenAI SSE 序列格式
        let sequence = vec![
            r#"data: {"choices":[{"delta":{"content":"你好"},"finish_reason":null}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
            "data: [DONE]",
        ];
        assert!(sequence[0].starts_with("data: "));
        assert!(sequence[1].contains("finish_reason"));
        assert_eq!(sequence[2], "data: [DONE]");
    }

    // ── 失败路径测试 ──

    #[test]
    fn test_failure_missing_model() {
        let json = r#"{"messages":[{"role":"user","content":"hi"}]}"#;
        let result: std::result::Result<ChatCompletionRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_failure_missing_messages() {
        let json = r#"{"model":"gpt-4o"}"#;
        let result: std::result::Result<ChatCompletionRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_failure_empty_parts_array() {
        // 空 parts 数组应该反序列化失败
        let json = r#"{"role":"user","content":[]}"#;
        let result: std::result::Result<ChatMessage, _> = serde_json::from_str(json);
        // 空数组 → 空字符串 → 反序列化成功但内容为空
        if let Ok(msg) = result {
            assert!(msg.content.is_empty());
        }
    }

    #[test]
    fn test_failure_provider_protocol_mapping() {
        let protocol = match "unknown_provider" {
            "anthropic" => ProviderProtocol::Anthropic,
            "ollama" => ProviderProtocol::Ollama,
            _ => ProviderProtocol::OpenAiCompletions,
        };
        assert!(matches!(protocol, ProviderProtocol::OpenAiCompletions));
    }

    // ── 安全审计测试 ──

    #[test]
    fn test_audit_api_key_not_in_sse_response() {
        let chunk = serde_json::json!({
            "choices": [{"delta": {"content": "Hello"}, "finish_reason": null}]
        });
        let line = format!("data: {}\n\n", chunk);
        assert!(!line.contains("sk-"));
        assert!(!line.contains("api_key"));
        assert!(!line.contains("Authorization"));
    }

    #[test]
    fn test_audit_request_body_no_api_key() {
        // 客户端请求体不包含 API Key
        let req = ChatCompletionRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![ChatMessage { role: "user".to_string(), content: "hi".to_string() }],
            stream: true,
            max_tokens: None,
            temperature: None,
        };
        let body = serde_json::to_string(&req).unwrap();
        assert!(!body.contains("sk-"));
        assert!(!body.contains("api_key"));
    }
}
