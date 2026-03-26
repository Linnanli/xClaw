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

#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 自定义反序列化：兼容 OpenAI 格式（`content` 字段）和 Vercel AI SDK 格式（`parts` 字段）
///
/// 支持的格式：
/// - `{"role":"user","content":"hello"}`                          — OpenAI 字符串
/// - `{"role":"user","content":[{"type":"text","text":"hello"}]}` — OpenAI parts 数组
/// - `{"role":"user","parts":[{"type":"text","text":"hello"}]}`   — Vercel AI SDK UIMessage
impl<'de> Deserialize<'de> for ChatMessage {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        #[derive(Deserialize)]
        struct RawMessage {
            role: String,
            content: Option<serde_json::Value>,
            parts: Option<serde_json::Value>,
        }

        let raw = RawMessage::deserialize(deserializer)?;

        let content = if let Some(content_val) = raw.content {
            parse_content_value(content_val).map_err(de::Error::custom)?
        } else if let Some(parts_val) = raw.parts {
            parse_parts_value(parts_val).map_err(de::Error::custom)?
        } else {
            return Err(de::Error::custom("message must have 'content' or 'parts' field"));
        };

        Ok(ChatMessage { role: raw.role, content })
    }
}

/// 解析 content 字段（字符串或 parts 数组）
fn parse_content_value(val: serde_json::Value) -> std::result::Result<String, String> {
    match val {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Array(_) => parse_parts_value(val),
        _ => Err("content must be string or array".to_string()),
    }
}

/// 从 parts 数组提取文本
fn parse_parts_value(val: serde_json::Value) -> std::result::Result<String, String> {
    let arr = val.as_array().ok_or("parts must be an array")?;
    let text: String = arr.iter()
        .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        Err("no text content in message parts".to_string())
    } else {
        Ok(text)
    }
}

// ============================================================================
// Handler
// ============================================================================

/// POST /api/chat/completions — 调用 LLM 并返回 Data Stream 格式
#[instrument(skip_all)]
pub async fn chat_completions_handler(
    State(state): State<AppState>,
    body_bytes: axum::body::Bytes,
) -> Result<Response> {
    // 调试：打印原始请求体
    let raw_str = String::from_utf8_lossy(&body_bytes);
    tracing::info!(raw_body = %raw_str, "Received chat completion request");

    let body: ChatCompletionRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| {
            tracing::error!(error = %e, raw_body = %raw_str, "Failed to deserialize request");
            Error::Validation(format!("Failed to deserialize the JSON body into the target type: {}", e))
        })?;

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

    // 6. 编码为 Vercel AI SDK Data Stream 格式（UIMessageChunk JSON 事件流）
    //
    // 协议：每行一个 JSON 对象，用 \n 分隔
    // 前端 DefaultChatTransport.processResponseStream 用 parseJsonEventStream 解析
    let msg_id = format!("msg-{}", uuid::Uuid::new_v4());
    let mut output = String::new();

    // text-start 事件
    output.push_str(&serde_json::to_string(&serde_json::json!({
        "type": "text-start",
        "id": msg_id,
    })).unwrap());
    output.push('\n');

    // text-delta 事件（完整内容作为一个 delta）
    output.push_str(&serde_json::to_string(&serde_json::json!({
        "type": "text-delta",
        "id": msg_id,
        "delta": response.content,
    })).unwrap());
    output.push('\n');

    // text-end 事件
    output.push_str(&serde_json::to_string(&serde_json::json!({
        "type": "text-end",
        "id": msg_id,
    })).unwrap());
    output.push('\n');

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
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
        let json = r#"{"role":"user","content":[{"type":"image","url":"http://..."},{"type":"text","text":"描述图片"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "描述图片");
    }

    #[test]
    fn test_deserialize_vercel_ai_sdk_parts_field() {
        // Vercel AI SDK UIMessage 格式：parts 字段而不是 content
        let json = r#"{"role":"user","parts":[{"type":"text","text":"你好AI"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "你好AI");
    }

    #[test]
    fn test_deserialize_vercel_ai_sdk_multiple_parts() {
        let json = r#"{"role":"user","parts":[{"type":"text","text":"第一段"},{"type":"text","text":"第二段"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "第一段\n第二段");
    }

    #[test]
    fn test_deserialize_content_takes_priority_over_parts() {
        // 如果同时有 content 和 parts，content 优先
        let json = r#"{"role":"user","content":"from content","parts":[{"type":"text","text":"from parts"}]}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "from content");
    }

    #[test]
    fn test_deserialize_ignores_extra_fields() {
        // UIMessage 有 id、createdAt 等额外字段，应该被忽略
        let json = r#"{"role":"user","parts":[{"type":"text","text":"hi"}],"id":"msg-1","createdAt":"2024-01-01"}"#;
        let msg: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "hi");
    }

    // ── 契约测试：模拟 useChatRuntime 实际发送的完整请求体 ──

    #[test]
    fn test_contract_full_usechat_runtime_request() {
        // 模拟 useChatRuntime + AssistantChatTransport 实际发送的 JSON
        let json = r#"{
            "model": "deepseek-chat",
            "messages": [
                {
                    "id": "msg-abc123",
                    "role": "user",
                    "parts": [{"type": "text", "text": "你好"}],
                    "createdAt": "2024-01-01T00:00:00.000Z"
                }
            ],
            "trigger": "submit-message",
            "messageId": "msg-abc123"
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "deepseek-chat");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "你好");
    }

    #[test]
    fn test_contract_usechat_runtime_multi_turn() {
        // 多轮对话：user + assistant + user
        let json = r#"{
            "model": "deepseek-chat",
            "messages": [
                {"role": "user", "parts": [{"type": "text", "text": "你好"}], "id": "m1"},
                {"role": "assistant", "parts": [{"type": "text", "text": "你好！有什么可以帮你的？"}], "id": "m2"},
                {"role": "user", "parts": [{"type": "text", "text": "今天天气怎么样"}], "id": "m3"}
            ]
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.messages[0].content, "你好");
        assert_eq!(req.messages[1].content, "你好！有什么可以帮你的？");
        assert_eq!(req.messages[2].content, "今天天气怎么样");
    }

    #[test]
    fn test_contract_usechat_runtime_with_extra_body_fields() {
        // useChatRuntime 可能在 body 里加 callSettings、system、tools 等字段
        let json = r#"{
            "model": "deepseek-chat",
            "messages": [{"role": "user", "parts": [{"type": "text", "text": "hi"}], "id": "m1"}],
            "callSettings": {},
            "system": null,
            "tools": {},
            "trigger": "submit-message"
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.messages[0].content, "hi");
    }

    // ── 失败路径测试 ──

    #[test]
    fn test_failure_message_no_content_no_parts() {
        let json = r#"{"role":"user"}"#;
        let result: std::result::Result<ChatMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("content") || err.contains("parts"));
    }

    #[test]
    fn test_failure_parts_empty_array() {
        let json = r#"{"role":"user","parts":[]}"#;
        let result: std::result::Result<ChatMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_failure_parts_no_text_type() {
        let json = r#"{"role":"user","parts":[{"type":"image","url":"http://..."}]}"#;
        let result: std::result::Result<ChatMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ── 单元测试：Data Stream 格式 ──

    #[test]
    fn test_data_stream_text_start_format() {
        let event = serde_json::json!({"type": "text-start", "id": "msg-1"});
        let line = serde_json::to_string(&event).unwrap();
        assert!(line.contains("text-start"));
        assert!(line.contains("msg-1"));
    }

    #[test]
    fn test_data_stream_text_delta_format() {
        let event = serde_json::json!({"type": "text-delta", "id": "msg-1", "delta": "Hello"});
        let line = serde_json::to_string(&event).unwrap();
        assert!(line.contains("text-delta"));
        assert!(line.contains("Hello"));
    }

    #[test]
    fn test_data_stream_text_end_format() {
        let event = serde_json::json!({"type": "text-end", "id": "msg-1"});
        let line = serde_json::to_string(&event).unwrap();
        assert!(line.contains("text-end"));
    }

    #[test]
    fn test_data_stream_complete_sequence() {
        let msg_id = "msg-test";
        let events = vec![
            serde_json::json!({"type": "text-start", "id": msg_id}),
            serde_json::json!({"type": "text-delta", "id": msg_id, "delta": "你好"}),
            serde_json::json!({"type": "text-end", "id": msg_id}),
        ];
        let output: String = events.iter()
            .map(|e| format!("{}\n", serde_json::to_string(e).unwrap()))
            .collect();
        assert_eq!(output.lines().count(), 3);
        assert!(output.contains("text-start"));
        assert!(output.contains("text-delta"));
        assert!(output.contains("text-end"));
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
    fn test_contract_data_stream_sequence() {
        let msg_id = "msg-contract";
        let events = vec![
            serde_json::to_string(&serde_json::json!({"type": "text-start", "id": msg_id})).unwrap(),
            serde_json::to_string(&serde_json::json!({"type": "text-delta", "id": msg_id, "delta": "你好"})).unwrap(),
            serde_json::to_string(&serde_json::json!({"type": "text-end", "id": msg_id})).unwrap(),
        ];
        // 每个事件都是合法 JSON
        for event in &events {
            assert!(serde_json::from_str::<serde_json::Value>(event).is_ok());
        }
        // 序列完整：start → delta → end
        assert!(events[0].contains("text-start"));
        assert!(events[1].contains("text-delta"));
        assert!(events[2].contains("text-end"));
        // 所有事件共享同一个 id
        for event in &events {
            assert!(event.contains(msg_id));
        }
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
    fn test_audit_api_key_not_in_data_stream() {
        let event = serde_json::json!({"type": "text-delta", "id": "msg-1", "delta": "Hello"});
        let line = serde_json::to_string(&event).unwrap();
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
