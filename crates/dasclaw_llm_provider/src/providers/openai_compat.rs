//! OpenAI Chat Completions 兼容客户端。
//!
//! 适配所有走 `/v1/chat/completions` 协议的厂商：OpenAI、xAI（Grok）、阿里云
//! DashScope（Qwen/Kimi/DeepSeek 等）、Moonshot Kimi、Groq、OpenRouter、Tinfoil、
//! Ollama 本地等。各厂商通过 [`OpenAiCompatConfig`] 预设区分（base URL + 请求体大小限制）。
//!
//! 本 client 同时实现 [`OpenAiCompatClient::complete`]（同步消息）与
//! [`OpenAiCompatClient::stream`]（流式消息）。流式实现把 OpenAI 增量
//! `chat.completion.chunk` 协议映射为 Anthropic 风格的 [`StreamEvent`] 序列
//! （`message_start` / `content_block_*` / `message_delta` / `message_stop`），
//! 让上层 ironclaw 用一套事件模型消费两类后端。
//!
//! # 与 `claw-code-api::providers::openai_compat` 的差异
//!
//! 1. **共享 [`RetryPolicy`]**：复用 [`crate::retry::RetryPolicy`]，与 [`super::AnthropicClient`]
//!    保持一致行为；新增对 `Retry-After` 的尊重（claw-code 完全忽略）。
//! 2. **错误强类型化**：HTTP 状态分发到 [`ApiError`] 各变体，上层免去字符串扫描。
//! 3. **去除应用层耦合**：不依赖 telemetry / prompt cache / sanitize_tool_message_pairing
//!    等 runtime 私货；schema 规范化与孤儿 tool_call 清理留给上层调用前完成。
//! 4. **无 env 副作用**：[`OpenAiCompatClient::new`] 直接接收 API key，不再 `from_env`；
//!    凭据采集由调用方负责。

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::retry::{parse_retry_after, RetryPolicy};
use crate::types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};

const REQUEST_ID_HEADER: &str = "request-id";
const ALT_REQUEST_ID_HEADER: &str = "x-request-id";
const RETRY_AFTER_HEADER: &str = "retry-after";

/// xAI 默认 base URL。
pub const DEFAULT_XAI_BASE_URL: &str = "https://api.x.ai/v1";
/// OpenAI 默认 base URL。
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
/// 阿里云 DashScope 兼容模式默认 base URL。
pub const DEFAULT_DASHSCOPE_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";

const XAI_MAX_REQUEST_BODY_BYTES: usize = 52_428_800; // 50MB
const OPENAI_MAX_REQUEST_BODY_BYTES: usize = 104_857_600; // 100MB
const DASHSCOPE_MAX_REQUEST_BODY_BYTES: usize = 6_291_456; // 6MB（DashScope 实测限制）

/// OpenAI-compat 客户端的厂商预设。
///
/// 通过 [`Self::openai`] / [`Self::xai`] / [`Self::dashscope`] 三个 const 构造器获得；
/// 调用方可经 [`OpenAiCompatClient::with_base_url`] 覆盖 `default_base_url` 以指向自托管端点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenAiCompatConfig {
    /// 厂商名（用于错误消息与遥测）。
    pub provider_name: &'static str,
    /// 默认 base URL（不含尾部 `/`）。
    pub default_base_url: &'static str,
    /// 请求体最大字节数（厂商侧硬限制）。
    pub max_request_body_bytes: usize,
}

impl OpenAiCompatConfig {
    /// xAI Grok 预设（`https://api.x.ai/v1`）。
    #[must_use]
    pub const fn xai() -> Self {
        Self {
            provider_name: "xai",
            default_base_url: DEFAULT_XAI_BASE_URL,
            max_request_body_bytes: XAI_MAX_REQUEST_BODY_BYTES,
        }
    }

    /// OpenAI 预设（`https://api.openai.com/v1`）。
    #[must_use]
    pub const fn openai() -> Self {
        Self {
            provider_name: "openai",
            default_base_url: DEFAULT_OPENAI_BASE_URL,
            max_request_body_bytes: OPENAI_MAX_REQUEST_BODY_BYTES,
        }
    }

    /// 阿里云 DashScope 兼容模式预设（Qwen / DeepSeek / Kimi 等）。
    #[must_use]
    pub const fn dashscope() -> Self {
        Self {
            provider_name: "dashscope",
            default_base_url: DEFAULT_DASHSCOPE_BASE_URL,
            max_request_body_bytes: DASHSCOPE_MAX_REQUEST_BODY_BYTES,
        }
    }
}

/// OpenAI Chat Completions 兼容客户端。
#[derive(Debug, Clone)]
pub struct OpenAiCompatClient {
    http: reqwest::Client,
    api_key: String,
    config: OpenAiCompatConfig,
    base_url: String,
    retry_policy: RetryPolicy,
}

impl OpenAiCompatClient {
    /// 用 API key + 厂商预设构造客户端。
    pub fn new(api_key: impl Into<String>, config: OpenAiCompatConfig) -> Result<Self, ApiError> {
        Ok(Self {
            http: crate::http::build_http_client_with(&crate::http::ProxyConfig::default())?,
            api_key: api_key.into(),
            config,
            base_url: config.default_base_url.to_string(),
            retry_policy: RetryPolicy::default(),
        })
    }

    /// 注入预构造的 `reqwest::Client`（如已配置代理 / 自定义 TLS）。
    #[must_use]
    pub fn with_http_client(mut self, http: reqwest::Client) -> Self {
        self.http = http;
        self
    }

    /// 覆盖 base URL。
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// 覆盖默认 [`RetryPolicy`]。
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// 当前 base URL（只读）。
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 当前厂商预设（只读）。
    #[must_use]
    pub const fn config(&self) -> OpenAiCompatConfig {
        self.config
    }

    /// 同步（非流式）发送消息请求。
    ///
    /// 内部强制将 `request.stream` 改写为 `false`；将 [`MessageRequest`]
    /// 翻译为 OpenAI Chat Completions 协议体后投递，再把响应映射回
    /// [`MessageResponse`]。
    pub async fn complete(&self, request: &MessageRequest) -> Result<MessageResponse, ApiError> {
        let request = MessageRequest {
            stream: false,
            ..request.clone()
        };
        let payload = build_chat_completion_request(&request, self.config);
        check_request_body_size(&payload, self.config)?;

        let response = self.send_with_retry(&payload).await?;
        let request_id = request_id_from_headers(response.headers());
        let body = response
            .text()
            .await
            .map_err(|err| ApiError::Transport(err.to_string()))?;

        // 部分 OpenAI-compat 后端把错误塞回 200 体的 `error` 字段而不是 HTTP 状态。
        if let Some(err) = detect_inline_error(&body, self.config.provider_name, &request_id) {
            return Err(err);
        }

        let parsed: ChatCompletionResponse =
            serde_json::from_str(&body).map_err(|err| ApiError::Provider {
                provider: self.config.provider_name.to_string(),
                reason: format!(
                    "failed to deserialize ChatCompletionResponse for model={}: {}",
                    request.model, err
                ),
                request_id: request_id.clone(),
            })?;

        let mut normalized = normalize_response(&request.model, parsed)?;
        if normalized.request_id.is_none() {
            normalized.request_id = request_id;
        }
        Ok(normalized)
    }

    /// 流式发送消息请求；返回 [`OpenAiCompatStream`] 增量推送 Anthropic 风格的
    /// [`StreamEvent`] 序列。
    ///
    /// 内部强制将 `request.stream` 改写为 `true`，并把 OpenAI 增量
    /// `chat.completion.chunk` 协议（`data: {...}\n\n` SSE 帧 + `[DONE]` 哨兵）
    /// 翻译为 Anthropic 6 类事件（`message_start` / `content_block_*` /
    /// `message_delta` / `message_stop`）。
    ///
    /// Tool 调用增量遵循 OpenAI 协议：每个 tool call 用 `index` 区分，名称在
    /// 第一帧给出，arguments 由后续帧逐片拼接 — 本实现把它映射为
    /// `content_block_start { tool_use, input: {} }` + 多次 `input_json_delta`
    /// + `content_block_stop`，与 Anthropic 一致。
    pub async fn stream(&self, request: &MessageRequest) -> Result<OpenAiCompatStream, ApiError> {
        let request = MessageRequest {
            stream: true,
            ..request.clone()
        };
        let payload = build_chat_completion_request(&request, self.config);
        check_request_body_size(&payload, self.config)?;

        let response = self.send_with_retry(&payload).await?;
        let request_id = request_id_from_headers(response.headers());
        Ok(OpenAiCompatStream {
            request_id,
            response,
            parser: OpenAiSseParser::new(self.config.provider_name, request.model.clone()),
            pending: VecDeque::new(),
            done: false,
            state: StreamState::new(request.model.clone()),
        })
    }

    async fn send_with_retry(&self, payload: &Value) -> Result<reqwest::Response, ApiError> {
        let mut attempt: u32 = 0;
        loop {
            let outcome = self.send_once(payload).await;
            match outcome {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let retryable = error.is_retryable();
                    if !retryable || attempt >= self.retry_policy.max_retries {
                        return Err(error);
                    }
                    let suggested = retry_after_from_error(&error);
                    attempt += 1;
                    let delay = self
                        .retry_policy
                        .delay_for_attempt(attempt, suggested)
                        .ok_or_else(|| ApiError::Provider {
                            provider: self.config.provider_name.to_string(),
                            reason: format!("retry backoff overflow at attempt {attempt}"),
                            request_id: error.request_id().map(ToOwned::to_owned),
                        })?;
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn send_once(&self, payload: &Value) -> Result<reqwest::Response, ApiError> {
        let url = chat_completions_endpoint(&self.base_url);
        let response = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .bearer_auth(&self.api_key)
            .json(payload)
            .send()
            .await
            .map_err(|err| ApiError::Transport(err.to_string()))?;
        expect_success(response, self.config.provider_name).await
    }
}

fn retry_after_from_error(error: &ApiError) -> Option<Duration> {
    match error {
        ApiError::RateLimited {
            retry_after_secs: Some(secs),
            ..
        } => Some(Duration::from_secs(*secs)),
        _ => None,
    }
}

fn request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .or_else(|| headers.get(ALT_REQUEST_ID_HEADER))
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

async fn expect_success(
    response: reqwest::Response,
    provider: &'static str,
) -> Result<reqwest::Response, ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let request_id = request_id_from_headers(response.headers());
    let retry_after = response
        .headers()
        .get(RETRY_AFTER_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_retry_after);

    let body = response.text().await.unwrap_or_default();
    let parsed_message = parse_openai_error_message(&body);
    let reason = parsed_message.unwrap_or_else(|| truncate_body(&body));

    Err(map_status_to_api_error(
        provider,
        status,
        reason,
        retry_after,
        request_id,
    ))
}

fn map_status_to_api_error(
    provider: &'static str,
    status: StatusCode,
    reason: String,
    retry_after: Option<Duration>,
    request_id: Option<String>,
) -> ApiError {
    let provider = provider.to_string();
    match status.as_u16() {
        429 => ApiError::RateLimited {
            provider,
            retry_after_secs: retry_after.map(|d| d.as_secs()),
            request_id,
        },
        401 | 403 => ApiError::AuthFailed {
            provider,
            reason,
            request_id,
        },
        400 => ApiError::BadRequest {
            provider,
            reason,
            request_id,
        },
        408 | 409 | 500 | 502 | 503 | 504 => ApiError::ServerError {
            provider,
            status: status.as_u16(),
            reason,
            request_id,
        },
        _ => ApiError::Provider {
            provider,
            reason: format!("unexpected status {status}: {reason}"),
            request_id,
        },
    }
}

fn detect_inline_error(
    body: &str,
    provider: &'static str,
    request_id: &Option<String>,
) -> Option<ApiError> {
    let raw: Value = serde_json::from_str(body).ok()?;
    let err_obj = raw.get("error")?;
    let message = err_obj
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("provider returned an error")
        .to_string();
    Some(ApiError::Provider {
        provider: provider.to_string(),
        reason: message,
        request_id: request_id.clone(),
    })
}

fn parse_openai_error_message(body: &str) -> Option<String> {
    let raw: Value = serde_json::from_str(body).ok()?;
    raw.get("error")
        .and_then(|err| err.get("message"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn truncate_body(body: &str) -> String {
    const MAX: usize = 200;
    if body.len() <= MAX {
        return body.to_string();
    }
    let safe_end = (0..=MAX)
        .rev()
        .find(|&i| body.is_char_boundary(i))
        .unwrap_or(0);
    format!("{}…(truncated, {} bytes)", &body[..safe_end], body.len())
}

fn check_request_body_size(payload: &Value, config: OpenAiCompatConfig) -> Result<(), ApiError> {
    let estimated = serde_json::to_vec(payload).map(|v| v.len()).unwrap_or(0);
    if estimated > config.max_request_body_bytes {
        return Err(ApiError::BadRequest {
            provider: config.provider_name.to_string(),
            reason: format!(
                "request body {estimated} bytes exceeds {} max {} bytes",
                config.provider_name, config.max_request_body_bytes
            ),
            request_id: None,
        });
    }
    Ok(())
}

/// 把 [`MessageRequest`] 翻译成 OpenAI Chat Completions 协议体。
///
/// 公开仅为了基准测试与契约测试需要；正常调用走 [`OpenAiCompatClient::complete`]。
#[must_use]
pub fn build_chat_completion_request(
    request: &MessageRequest,
    _config: OpenAiCompatConfig,
) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = request.system.as_ref().filter(|value| !value.is_empty()) {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for message in &request.messages {
        messages.extend(translate_message(message));
    }

    // gpt-5* 系列改用 max_completion_tokens；其余沿用 max_tokens。
    let max_tokens_key = if request.model.starts_with("gpt-5") {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };

    let mut payload = json!({
        "model": request.model,
        max_tokens_key: request.max_tokens,
        "messages": messages,
        "stream": request.stream,
    });

    if let Some(tools) = &request.tools {
        payload["tools"] =
            Value::Array(tools.iter().map(openai_tool_definition).collect::<Vec<_>>());
    }
    if let Some(tool_choice) = &request.tool_choice {
        payload["tool_choice"] = openai_tool_choice(tool_choice);
    }

    // Reasoning models（o1 / o3 / o4 / grok-3-mini）拒绝 sampling 参数；本 crate
    // 不在 wire 层做静默过滤——上层调用方应根据模型选择性传入。
    if let Some(temperature) = request.temperature {
        payload["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.top_p {
        payload["top_p"] = json!(top_p);
    }
    if let Some(frequency_penalty) = request.frequency_penalty {
        payload["frequency_penalty"] = json!(frequency_penalty);
    }
    if let Some(presence_penalty) = request.presence_penalty {
        payload["presence_penalty"] = json!(presence_penalty);
    }
    if let Some(stop) = request.stop.as_ref().filter(|s| !s.is_empty()) {
        payload["stop"] = json!(stop);
    }
    if let Some(effort) = &request.reasoning_effort {
        payload["reasoning_effort"] = json!(effort);
    }

    payload
}

fn translate_message(message: &InputMessage) -> Vec<Value> {
    match message.role.as_str() {
        "assistant" => translate_assistant_message(&message.content),
        _ => translate_user_message(&message.content),
    }
}

fn translate_assistant_message(blocks: &[InputContentBlock]) -> Vec<Value> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for block in blocks {
        match block {
            InputContentBlock::Text { text: value } => text.push_str(value),
            InputContentBlock::ToolUse { id, name, input } => tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": input.to_string(),
                }
            })),
            InputContentBlock::ToolResult { .. } => {}
        }
    }
    if text.is_empty() && tool_calls.is_empty() {
        return Vec::new();
    }
    let mut msg = json!({
        "role": "assistant",
        "content": (!text.is_empty()).then_some(text),
    });
    if !tool_calls.is_empty() {
        msg["tool_calls"] = json!(tool_calls);
    }
    vec![msg]
}

fn translate_user_message(blocks: &[InputContentBlock]) -> Vec<Value> {
    blocks
        .iter()
        .filter_map(|block| match block {
            InputContentBlock::Text { text } => Some(json!({ "role": "user", "content": text })),
            InputContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(json!({
                "role": "tool",
                "tool_call_id": tool_use_id,
                "content": flatten_tool_result_content(content),
                "is_error": *is_error,
            })),
            InputContentBlock::ToolUse { .. } => None,
        })
        .collect()
}

fn flatten_tool_result_content(content: &[ToolResultContentBlock]) -> String {
    let total_len: usize = content
        .iter()
        .map(|block| match block {
            ToolResultContentBlock::Text { text } => text.len(),
            ToolResultContentBlock::Json { value } => value.to_string().len(),
        })
        .sum();
    let capacity = total_len + content.len().saturating_sub(1);
    let mut result = String::with_capacity(capacity);
    for (i, block) in content.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        match block {
            ToolResultContentBlock::Text { text } => result.push_str(text),
            ToolResultContentBlock::Json { value } => result.push_str(&value.to_string()),
        }
    }
    result
}

fn openai_tool_definition(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        }
    })
}

fn openai_tool_choice(tool_choice: &ToolChoice) -> Value {
    match tool_choice {
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::Any => Value::String("required".to_string()),
        ToolChoice::Tool { name } => json!({
            "type": "function",
            "function": { "name": name },
        }),
    }
}

fn normalize_response(
    request_model: &str,
    response: ChatCompletionResponse,
) -> Result<MessageResponse, ApiError> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ApiError::Provider {
            provider: "openai".to_string(),
            reason: "chat completion response missing choices".to_string(),
            request_id: None,
        })?;

    let mut content = Vec::new();
    if let Some(reasoning) = choice
        .message
        .reasoning_content
        .filter(|value| !value.is_empty())
    {
        content.push(OutputContentBlock::Thinking {
            thinking: reasoning,
            signature: None,
        });
    }
    if let Some(text) = choice.message.content.filter(|value| !value.is_empty()) {
        content.push(OutputContentBlock::Text { text });
    }
    for tool_call in choice.message.tool_calls {
        content.push(OutputContentBlock::ToolUse {
            id: tool_call.id,
            name: tool_call.function.name,
            input: parse_tool_arguments(&tool_call.function.arguments),
        });
    }

    let model = if response.model.is_empty() {
        request_model.to_string()
    } else {
        response.model
    };

    Ok(MessageResponse {
        id: response.id,
        kind: "message".to_string(),
        role: choice.message.role,
        content,
        model,
        stop_reason: choice
            .finish_reason
            .map(|value| normalize_finish_reason(&value)),
        stop_sequence: None,
        usage: Usage {
            input_tokens: response
                .usage
                .as_ref()
                .map_or(0, |usage| usage.prompt_tokens),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            output_tokens: response
                .usage
                .as_ref()
                .map_or(0, |usage| usage.completion_tokens),
        },
        request_id: None,
    })
}

fn parse_tool_arguments(arguments: &str) -> Value {
    serde_json::from_str(arguments).unwrap_or_else(|_| json!({ "raw": arguments }))
}

fn normalize_finish_reason(value: &str) -> String {
    match value {
        "stop" => "end_turn",
        "tool_calls" => "tool_use",
        other => other,
    }
    .to_string()
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    id: String,
    #[serde(default)]
    model: String,
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ResponseToolCall>,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    function: ResponseToolFunction,
}

#[derive(Debug, Deserialize)]
struct ResponseToolFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

// ---------- Streaming ----------

/// OpenAI Chat Completions 流式响应；由 [`OpenAiCompatClient::stream`] 创建。
///
/// 增量协议为 OpenAI `chat.completion.chunk` 序列（`data: {...}\n\n` SSE 帧 +
/// `[DONE]` 哨兵），但本类型对外暴露 Anthropic 风格的 [`StreamEvent`]——上层
/// ironclaw 用同一套 `next_event()` 消费 anthropic / openai_compat 两类后端。
#[derive(Debug)]
pub struct OpenAiCompatStream {
    request_id: Option<String>,
    response: reqwest::Response,
    parser: OpenAiSseParser,
    pending: VecDeque<StreamEvent>,
    done: bool,
    state: StreamState,
}

impl OpenAiCompatStream {
    /// 上游返回的 trace ID（如有）。
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }

    /// 增量拉取下一个 [`StreamEvent`]；流结束时返回 `Ok(None)`。
    ///
    /// 内部状态机：先消费 HTTP body 字节 → SSE 帧 → `ChatCompletionChunk` →
    /// 经 [`StreamState::ingest_chunk`] 翻译为 Anthropic 风格事件队列。
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            if self.done {
                let trailing = self.state.finish()?;
                if trailing.is_empty() {
                    return Ok(None);
                }
                self.pending.extend(trailing);
                continue;
            }
            match self
                .response
                .chunk()
                .await
                .map_err(|err| ApiError::StreamInterrupted(err.to_string()))?
            {
                Some(bytes) => {
                    for chunk in self.parser.push(&bytes)? {
                        self.pending.extend(self.state.ingest_chunk(chunk)?);
                    }
                }
                None => {
                    self.done = true;
                }
            }
        }
    }
}

/// OpenAI 形态 SSE 解析器：识别 `data: {...}\n\n` 帧与 `data: [DONE]` 哨兵。
///
/// 与 [`crate::sse::SseParser`]（Anthropic 形态：`event: foo\ndata: {...}`）的差异：
/// OpenAI 不带 `event:` 字段，所有事件都是裸 JSON chunk；只有一个
/// `[DONE]` 哨兵字符串作为流结束标记（不是 JSON）。
#[derive(Debug)]
struct OpenAiSseParser {
    provider: &'static str,
    model: String,
    buffer: Vec<u8>,
}

impl OpenAiSseParser {
    fn new(provider: &'static str, model: String) -> Self {
        Self {
            provider,
            model,
            buffer: Vec::new(),
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<Vec<ChatCompletionChunk>, ApiError> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(frame) = take_frame(&mut self.buffer) {
            if let Some(event) = self.parse_frame(&frame)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    fn parse_frame(&self, frame: &str) -> Result<Option<ChatCompletionChunk>, ApiError> {
        let mut payload = String::new();
        for line in frame.split('\n') {
            let line = line.strip_prefix('\r').unwrap_or(line);
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let Some(rest) = line.strip_prefix("data:") else {
                continue;
            };
            let value = rest.strip_prefix(' ').unwrap_or(rest);
            if value == "[DONE]" {
                return Ok(None);
            }
            if !payload.is_empty() {
                payload.push('\n');
            }
            payload.push_str(value);
        }
        if payload.is_empty() {
            return Ok(None);
        }
        serde_json::from_str::<ChatCompletionChunk>(&payload)
            .map(Some)
            .map_err(|err| {
                ApiError::MalformedSseFrame(format!(
                    "{}: failed to deserialize chat.completion.chunk for model={}: {err}",
                    self.provider, self.model
                ))
            })
    }
}

/// 从 buffer 中切出下一个 SSE 帧（以 `\n\n` 为分隔；兼容 `\r\n\r\n`）。
fn take_frame(buffer: &mut Vec<u8>) -> Option<String> {
    let needle_lf = b"\n\n";
    let needle_crlf = b"\r\n\r\n";
    let lf_idx = buffer.windows(needle_lf.len()).position(|w| w == needle_lf);
    let crlf_idx = buffer
        .windows(needle_crlf.len())
        .position(|w| w == needle_crlf);
    let (split_at, sep_len) = match (lf_idx, crlf_idx) {
        (Some(lf), Some(crlf)) if crlf <= lf => (crlf, needle_crlf.len()),
        (Some(lf), _) => (lf, needle_lf.len()),
        (None, Some(crlf)) => (crlf, needle_crlf.len()),
        (None, None) => return None,
    };
    let frame_bytes: Vec<u8> = buffer.drain(..split_at).collect();
    buffer.drain(..sep_len);
    Some(String::from_utf8_lossy(&frame_bytes).into_owned())
}

/// 把 OpenAI `chat.completion.chunk` 流翻译成 Anthropic 风格 [`StreamEvent`] 序列的状态机。
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct StreamState {
    model: String,
    message_started: bool,
    text_started: bool,
    text_finished: bool,
    finished: bool,
    stop_reason: Option<String>,
    usage: Option<Usage>,
    tool_calls: BTreeMap<u32, ToolCallState>,
}

impl StreamState {
    fn new(model: String) -> Self {
        Self {
            model,
            message_started: false,
            text_started: false,
            text_finished: false,
            finished: false,
            stop_reason: None,
            usage: None,
            tool_calls: BTreeMap::new(),
        }
    }

    fn ingest_chunk(&mut self, chunk: ChatCompletionChunk) -> Result<Vec<StreamEvent>, ApiError> {
        let mut events = Vec::new();
        if !self.message_started {
            self.message_started = true;
            events.push(StreamEvent::MessageStart(MessageStartEvent {
                message: MessageResponse {
                    id: chunk.id.clone(),
                    kind: "message".to_string(),
                    role: "assistant".to_string(),
                    content: Vec::new(),
                    model: chunk.model.clone().unwrap_or_else(|| self.model.clone()),
                    stop_reason: None,
                    stop_sequence: None,
                    usage: Usage {
                        input_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                        output_tokens: 0,
                    },
                    request_id: None,
                },
            }));
        }

        if let Some(usage) = chunk.usage {
            self.usage = Some(Usage {
                input_tokens: usage.prompt_tokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                output_tokens: usage.completion_tokens,
            });
        }

        for choice in chunk.choices {
            if let Some(content) = choice.delta.content.filter(|value| !value.is_empty()) {
                if !self.text_started {
                    self.text_started = true;
                    events.push(StreamEvent::ContentBlockStart(ContentBlockStartEvent {
                        index: 0,
                        content_block: OutputContentBlock::Text {
                            text: String::new(),
                        },
                    }));
                }
                events.push(StreamEvent::ContentBlockDelta(ContentBlockDeltaEvent {
                    index: 0,
                    delta: ContentBlockDelta::TextDelta { text: content },
                }));
            }

            for tool_call in choice.delta.tool_calls {
                let state = self.tool_calls.entry(tool_call.index).or_default();
                state.apply(tool_call);
                let block_index = state.block_index();
                if !state.started {
                    if let Some(start_event) = state.start_event() {
                        state.started = true;
                        events.push(StreamEvent::ContentBlockStart(start_event));
                    } else {
                        continue;
                    }
                }
                if let Some(delta_event) = state.delta_event() {
                    events.push(StreamEvent::ContentBlockDelta(delta_event));
                }
                if choice.finish_reason.as_deref() == Some("tool_calls") && !state.stopped {
                    state.stopped = true;
                    events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                        index: block_index,
                    }));
                }
            }

            if let Some(finish_reason) = choice.finish_reason {
                self.stop_reason = Some(normalize_finish_reason(&finish_reason));
                if finish_reason == "tool_calls" {
                    for state in self.tool_calls.values_mut() {
                        if state.started && !state.stopped {
                            state.stopped = true;
                            events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                                index: state.block_index(),
                            }));
                        }
                    }
                }
            }
        }

        Ok(events)
    }

    /// 在底层 HTTP body 结束时收尾：补 `content_block_stop` / `message_delta` /
    /// `message_stop`。幂等 — 多次调用只在第一次产出事件。
    fn finish(&mut self) -> Result<Vec<StreamEvent>, ApiError> {
        if self.finished {
            return Ok(Vec::new());
        }
        self.finished = true;

        let mut events = Vec::new();
        if self.text_started && !self.text_finished {
            self.text_finished = true;
            events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                index: 0,
            }));
        }

        for state in self.tool_calls.values_mut() {
            if !state.started {
                if let Some(start_event) = state.start_event() {
                    state.started = true;
                    events.push(StreamEvent::ContentBlockStart(start_event));
                    if let Some(delta_event) = state.delta_event() {
                        events.push(StreamEvent::ContentBlockDelta(delta_event));
                    }
                }
            }
            if state.started && !state.stopped {
                state.stopped = true;
                events.push(StreamEvent::ContentBlockStop(ContentBlockStopEvent {
                    index: state.block_index(),
                }));
            }
        }

        if self.message_started {
            events.push(StreamEvent::MessageDelta(MessageDeltaEvent {
                delta: MessageDelta {
                    stop_reason: Some(
                        self.stop_reason
                            .clone()
                            .unwrap_or_else(|| "end_turn".to_string()),
                    ),
                    stop_sequence: None,
                },
                usage: self.usage.clone().unwrap_or(Usage {
                    input_tokens: 0,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                    output_tokens: 0,
                }),
            }));
            events.push(StreamEvent::MessageStop(MessageStopEvent {}));
        }
        Ok(events)
    }
}

/// 单个 OpenAI tool call 的累积状态。
///
/// OpenAI 流式协议中，同一个 tool call 跨多个 chunk 切片：第一个 chunk 给出 `id` +
/// `function.name`，后续 chunk 仅切片 `function.arguments`。`block_index` 取
/// `openai_index + 1`，把索引 0 留给可能存在的 text content block。
#[derive(Debug, Default)]
struct ToolCallState {
    openai_index: u32,
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    emitted_len: usize,
    started: bool,
    stopped: bool,
}

impl ToolCallState {
    fn apply(&mut self, tool_call: DeltaToolCall) {
        self.openai_index = tool_call.index;
        if let Some(id) = tool_call.id {
            self.id = Some(id);
        }
        if let Some(name) = tool_call.function.name {
            self.name = Some(name);
        }
        if let Some(arguments) = tool_call.function.arguments {
            self.arguments.push_str(&arguments);
        }
    }

    const fn block_index(&self) -> u32 {
        self.openai_index + 1
    }

    fn start_event(&self) -> Option<ContentBlockStartEvent> {
        let name = self.name.clone()?;
        let id = self
            .id
            .clone()
            .unwrap_or_else(|| format!("tool_call_{}", self.openai_index));
        Some(ContentBlockStartEvent {
            index: self.block_index(),
            content_block: OutputContentBlock::ToolUse {
                id,
                name,
                input: json!({}),
            },
        })
    }

    fn delta_event(&mut self) -> Option<ContentBlockDeltaEvent> {
        if self.emitted_len >= self.arguments.len() {
            return None;
        }
        let delta = self.arguments[self.emitted_len..].to_string();
        self.emitted_len = self.arguments.len();
        Some(ContentBlockDeltaEvent {
            index: self.block_index(),
            delta: ContentBlockDelta::InputJsonDelta {
                partial_json: delta,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    id: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_as_empty_vec")]
    tool_calls: Vec<DeltaToolCall>,
}

#[derive(Debug, Deserialize)]
struct DeltaToolCall {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: DeltaFunction,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// 部分 OpenAI-compat 厂商把 `tool_calls` 显式写成 `null` 而不是省略字段或用 `[]`。
/// `#[serde(default)]` 仅处理缺失键，处理不了显式 null —— 本反序列化器把
/// `null` 视为 `[]`。
fn deserialize_null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InputMessage, MessageRequest};

    #[test]
    fn presets_have_distinct_provider_names() {
        assert_eq!(OpenAiCompatConfig::openai().provider_name, "openai");
        assert_eq!(OpenAiCompatConfig::xai().provider_name, "xai");
        assert_eq!(OpenAiCompatConfig::dashscope().provider_name, "dashscope");
    }

    #[test]
    fn chat_completions_endpoint_appends_path() {
        assert_eq!(
            chat_completions_endpoint("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn chat_completions_endpoint_idempotent_when_already_present() {
        assert_eq!(
            chat_completions_endpoint("https://gateway.example.com/v1/chat/completions"),
            "https://gateway.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn build_request_uses_max_completion_tokens_for_gpt_5() {
        let request = MessageRequest {
            model: "gpt-5-mini".to_string(),
            max_tokens: 32,
            messages: vec![InputMessage::user_text("hi")],
            ..MessageRequest::default()
        };
        let payload = build_chat_completion_request(&request, OpenAiCompatConfig::openai());
        assert!(payload.get("max_completion_tokens").is_some());
        assert!(payload.get("max_tokens").is_none());
    }

    #[test]
    fn build_request_uses_max_tokens_for_other_models() {
        let request = MessageRequest {
            model: "gpt-4o".to_string(),
            max_tokens: 32,
            messages: vec![InputMessage::user_text("hi")],
            ..MessageRequest::default()
        };
        let payload = build_chat_completion_request(&request, OpenAiCompatConfig::openai());
        assert!(payload.get("max_tokens").is_some());
        assert!(payload.get("max_completion_tokens").is_none());
    }

    #[test]
    fn build_request_emits_system_message_when_present() {
        let request = MessageRequest {
            model: "gpt-4o".to_string(),
            max_tokens: 16,
            messages: vec![InputMessage::user_text("hi")],
            system: Some("you are a tester".to_string()),
            ..MessageRequest::default()
        };
        let payload = build_chat_completion_request(&request, OpenAiCompatConfig::openai());
        let messages = payload["messages"].as_array().expect("messages array");
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "you are a tester");
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn translate_assistant_keeps_text_and_tool_calls() {
        let msg = InputMessage {
            role: "assistant".to_string(),
            content: vec![
                InputContentBlock::Text {
                    text: "calling".to_string(),
                },
                InputContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "lookup".to_string(),
                    input: json!({ "q": "foo" }),
                },
            ],
        };
        let translated = translate_message(&msg);
        assert_eq!(translated.len(), 1);
        assert_eq!(translated[0]["role"], "assistant");
        assert_eq!(translated[0]["content"], "calling");
        assert_eq!(translated[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(translated[0]["tool_calls"][0]["function"]["name"], "lookup");
    }

    #[test]
    fn translate_user_handles_tool_result() {
        let msg = InputMessage {
            role: "user".to_string(),
            content: vec![InputContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: vec![ToolResultContentBlock::Text {
                    text: "result".to_string(),
                }],
                is_error: false,
            }],
        };
        let translated = translate_message(&msg);
        assert_eq!(translated.len(), 1);
        assert_eq!(translated[0]["role"], "tool");
        assert_eq!(translated[0]["tool_call_id"], "call_1");
        assert_eq!(translated[0]["content"], "result");
    }

    #[test]
    fn map_status_429_yields_rate_limited() {
        let err = map_status_to_api_error(
            "openai",
            StatusCode::TOO_MANY_REQUESTS,
            "slow".to_string(),
            Some(Duration::from_secs(3)),
            Some("req-1".to_string()),
        );
        match err {
            ApiError::RateLimited {
                provider,
                retry_after_secs,
                ..
            } => {
                assert_eq!(provider, "openai");
                assert_eq!(retry_after_secs, Some(3));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn map_status_401_yields_auth_failed() {
        let err = map_status_to_api_error(
            "openai",
            StatusCode::UNAUTHORIZED,
            "bad key".to_string(),
            None,
            None,
        );
        assert!(matches!(err, ApiError::AuthFailed { .. }));
    }

    #[test]
    fn map_status_500_yields_server_error() {
        let err = map_status_to_api_error(
            "openai",
            StatusCode::INTERNAL_SERVER_ERROR,
            "boom".to_string(),
            None,
            None,
        );
        assert!(matches!(err, ApiError::ServerError { status: 500, .. }));
    }

    #[test]
    fn detect_inline_error_recognizes_envelope() {
        let body = r#"{"error":{"message":"insufficient quota","type":"billing"}}"#;
        let err = detect_inline_error(body, "openai", &Some("req-9".to_string())).expect("error");
        match err {
            ApiError::Provider {
                provider, reason, ..
            } => {
                assert_eq!(provider, "openai");
                assert_eq!(reason, "insufficient quota");
            }
            other => panic!("expected Provider, got {other:?}"),
        }
    }

    #[test]
    fn check_request_body_size_rejects_oversized() {
        let huge = json!({ "blob": "x".repeat(10 * 1024 * 1024) });
        let result = check_request_body_size(&huge, OpenAiCompatConfig::dashscope());
        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[test]
    fn normalize_response_promotes_reasoning_to_thinking_block() {
        let body = json!({
            "id": "chatcmpl-1",
            "model": "qwen-max",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "answer",
                    "reasoning_content": "step 1, step 2"
                },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 5, "completion_tokens": 10 }
        });
        let parsed: ChatCompletionResponse = serde_json::from_value(body).expect("parses");
        let resp = normalize_response("qwen-max", parsed).expect("normalizes");
        assert_eq!(resp.content.len(), 2);
        assert!(matches!(
            resp.content[0],
            OutputContentBlock::Thinking { .. }
        ));
        assert!(matches!(resp.content[1], OutputContentBlock::Text { .. }));
        assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(resp.usage.input_tokens, 5);
        assert_eq!(resp.usage.output_tokens, 10);
    }
}
