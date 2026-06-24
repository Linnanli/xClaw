//! Codex ChatGPT Responses API provider.
//!
//! Implements `LlmProvider` by speaking the OpenAI Responses API protocol
//! (`POST /responses`) used by the ChatGPT backend at
//! `chatgpt.com/backend-api/codex`. This bypasses the standard Chat
//! Completions path, which is incompatible with this endpoint.
//!
//! # Warning
//!
//! The ChatGPT backend endpoint (`chatgpt.com/backend-api/codex`) is a
//! **private, undocumented API**. Using subscriber OAuth tokens from a
//! third-party application may violate the token's intended scope or
//! OpenAI's Terms of Service. This feature is provided as-is for
//! convenience and may break without notice.

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use reqwest::Client;
use rust_decimal::Decimal;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, mpsc};

use super::codex_auth;
use crate::provider::error::LlmError;

use super::provider::{
    ChatMessage, CompletionRequest, CompletionResponse, ContentPart, FinishReason, LlmPlanStep,
    LlmPlanStepStatus, LlmProvider, LlmStream, LlmStreamEvent, Role, ToolCall,
    ToolCompletionRequest, ToolCompletionResponse, ToolDefinition,
};

/// Provider that speaks the Responses API protocol against the ChatGPT backend.
pub struct CodexChatGptProvider {
    client: Client,
    base_url: String,
    api_key: RwLock<SecretString>,
    /// User-configured model name (or empty/"default" for auto-detect).
    configured_model: String,
    /// Lazily resolved model name (populated on first LLM call).
    resolved_model: tokio::sync::OnceCell<String>,
    /// OAuth refresh token for automatic 401 retry.
    refresh_token: Option<SecretString>,
    /// Path to auth.json for persisting refreshed tokens.
    auth_path: Option<PathBuf>,
    /// Timeout for actual `/responses` requests.
    request_timeout: Duration,
    /// Prevent concurrent 401 handlers from racing the same refresh token.
    refresh_lock: Mutex<()>,
}

impl CodexChatGptProvider {
    #[cfg(test)]
    fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: RwLock::new(SecretString::from(api_key.to_string())),
            configured_model: model.to_string(),
            resolved_model: tokio::sync::OnceCell::const_new(),
            refresh_token: None,
            auth_path: None,
            request_timeout: Duration::from_secs(120),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Create a provider with lazy model detection.
    ///
    /// The model is **not** resolved during construction. Instead, it is
    /// resolved on the first LLM call via [`resolve_model`], avoiding the
    /// need for `block_in_place` / `block_on` during provider setup.
    ///
    /// **Model selection priority** (applied at resolution time):
    /// 1. If `configured_model` is non-empty, validate it against the
    ///    `/models` endpoint. If it isn't in the supported list, log a
    ///    warning with available models and fall back to the top model.
    /// 2. If `configured_model` is empty (or a generic placeholder like
    ///    "default"), auto-detect the highest-priority model from the API.
    pub fn with_lazy_model(
        base_url: &str,
        api_key: SecretString,
        configured_model: &str,
        refresh_token: Option<SecretString>,
        auth_path: Option<PathBuf>,
        request_timeout_secs: u64,
    ) -> Self {
        tracing::warn!(
            "Codex ChatGPT provider uses a private, undocumented API \
             (chatgpt.com/backend-api/codex). This may violate OpenAI's \
             Terms of Service and could break without notice."
        );

        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: RwLock::new(api_key),
            configured_model: configured_model.to_string(),
            resolved_model: tokio::sync::OnceCell::const_new(),
            refresh_token,
            auth_path,
            request_timeout: Duration::from_secs(request_timeout_secs),
            refresh_lock: Mutex::new(()),
        }
    }

    /// Resolve the model to use, lazily on first call.
    ///
    /// Uses `OnceCell` so the `/models` fetch happens at most once.
    async fn resolve_model(&self) -> &str {
        self.resolved_model
            .get_or_init(|| async {
                let api_key = self.api_key.read().await.clone();
                let available = Self::fetch_available_models(&self.client, &self.base_url, &api_key)
                    .await;

                let configured = &self.configured_model;
                if !configured.is_empty() && configured != "default" {
                    // User explicitly configured a model — validate it
                    if available.is_empty() {
                        tracing::warn!(
                            "Could not fetch model list; using configured model '{configured}'"
                        );
                        return configured.clone();
                    }
                    if available.iter().any(|m| m == configured) {
                        tracing::info!(model = %configured, "Codex ChatGPT: using configured model");
                        return configured.clone();
                    }
                    tracing::warn!(
                        configured = %configured,
                        available = ?available,
                        "Configured model not found in supported list, falling back to top model"
                    );
                    available
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| configured.clone())
                } else {
                    // No user preference — auto-detect
                    if let Some(top) = available.into_iter().next() {
                        tracing::info!(model = %top, "Codex ChatGPT: auto-detected model");
                        top
                    } else {
                        tracing::warn!(
                            "Could not auto-detect model, using fallback '{configured}'"
                        );
                        configured.clone()
                    }
                }
            })
            .await
    }

    /// Query `/models?client_version=0.111.0` and return the list of available
    /// model slugs, ordered by priority (highest first).
    async fn fetch_available_models(
        client: &Client,
        base_url: &str,
        api_key: &SecretString,
    ) -> Vec<String> {
        let url = format!("{base_url}/models?client_version=0.111.0");
        let resp = match client
            .get(&url)
            .bearer_auth(api_key.expose_secret())
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to fetch Codex models: {e}");
                return Vec::new();
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Failed to fetch Codex models");
            return Vec::new();
        }
        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        // The response has { "models": [ { "slug": "...", ... }, ... ] }
        body.get("models")
            .and_then(|m| m.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        m.get("slug")
                            .and_then(|s| s.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Convert IronClaw messages to Responses API request JSON.
    fn build_request_body(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Value {
        // Extract system instructions
        let instructions: String = messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        // Convert non-system messages to Responses API input items
        let input: Vec<Value> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .flat_map(Self::message_to_input_items)
            .collect();

        // Convert tool definitions
        let api_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();

        let mut body = json!({
            "model": model,
            "instructions": instructions,
            "input": input,
            "stream": true,
            "store": false,
        });

        if !api_tools.is_empty() {
            body["tools"] = json!(api_tools);
            body["tool_choice"] = json!(tool_choice.unwrap_or("auto"));
        }

        body
    }

    /// Convert a single ChatMessage to one or more Responses API input items.
    fn message_to_input_items(msg: &ChatMessage) -> Vec<Value> {
        let mut items = Vec::new();

        match msg.role {
            Role::User => {
                // Build content array: if content_parts is populated, use it
                // to include multimodal content (images). Otherwise fall back
                // to the plain text content field.
                let content = if !msg.content_parts.is_empty() {
                    msg.content_parts
                        .iter()
                        .map(|part| match part {
                            ContentPart::Text { text } => json!({
                                "type": "input_text",
                                "text": text,
                            }),
                            ContentPart::ImageUrl { image_url } => json!({
                                "type": "input_image",
                                "image_url": image_url.url,
                            }),
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![json!({
                        "type": "input_text",
                        "text": msg.content,
                    })]
                };

                items.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": content,
                }));
            }
            Role::Assistant => {
                // If the assistant message has tool calls, emit function_call items
                if let Some(ref tool_calls) = msg.tool_calls {
                    // Emit the assistant text as a message if non-empty
                    if !msg.content.is_empty() {
                        items.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": [{
                                "type": "output_text",
                                "text": msg.content,
                            }],
                        }));
                    }
                    for tc in tool_calls {
                        let args = if tc.arguments.is_string() {
                            tc.arguments.as_str().unwrap_or("{}").to_string()
                        } else {
                            serde_json::to_string(&tc.arguments).unwrap_or_default()
                        };
                        items.push(json!({
                            "type": "function_call",
                            "name": tc.name,
                            "arguments": args,
                            "call_id": tc.id,
                        }));
                    }
                } else {
                    items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": msg.content,
                        }],
                    }));
                }
            }
            Role::Tool => {
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": msg.tool_call_id.as_deref().unwrap_or(""),
                    "output": msg.content,
                }));
            }
            Role::System => {
                // System messages are handled via `instructions` field
            }
        }

        items
    }

    /// Send a request and parse the SSE response.
    ///
    /// On HTTP 401, if a refresh token is available, attempts to refresh
    /// the access token and retry the request once.
    async fn send_responses_request(&self, body: &Value) -> Result<reqwest::Response, LlmError> {
        let url = format!("{}/responses", self.base_url);

        tracing::debug!(
            url = %url,
            model = %body.get("model").and_then(|m| m.as_str()).unwrap_or("?"),
            "Codex ChatGPT: sending request"
        );

        let api_key = self.api_key.read().await.clone();
        let resp =
            Self::send_http_request(&self.client, &url, &api_key, &body, self.request_timeout)
                .await?;

        let status = resp.status();
        if status.as_u16() == 401 {
            // Attempt token refresh if we have a refresh token
            if let Some(ref rt) = self.refresh_token {
                let _refresh_guard = self.refresh_lock.lock().await;
                let current_token = self.api_key.read().await.clone();

                if current_token.expose_secret() != api_key.expose_secret() {
                    tracing::info!("Received 401, but another request already refreshed the token");
                    let retry_resp = Self::send_http_request(
                        &self.client,
                        &url,
                        &current_token,
                        &body,
                        self.request_timeout,
                    )
                    .await?;
                    let retry_status = retry_resp.status();
                    if !retry_status.is_success() {
                        let body_text =
                            tokio::time::timeout(Duration::from_secs(5), retry_resp.text())
                                .await
                                .unwrap_or(Ok(String::new()))
                                .unwrap_or_default();
                        return Err(LlmError::RequestFailed {
                            provider: "codex_chatgpt".to_string(),
                            reason: format!(
                                "HTTP {retry_status} from {url} (after concurrent token refresh): {body_text}"
                            ),
                        });
                    }
                    return Ok(retry_resp);
                }

                tracing::info!("Received 401, attempting token refresh");
                if let Some(new_token) =
                    codex_auth::refresh_access_token(&self.client, rt, self.auth_path.as_deref())
                        .await
                {
                    // Update stored api_key
                    *self.api_key.write().await = new_token.clone();
                    tracing::info!("Token refreshed, retrying request");

                    // Retry the request with the new token
                    let retry_resp = Self::send_http_request(
                        &self.client,
                        &url,
                        &new_token,
                        &body,
                        self.request_timeout,
                    )
                    .await?;

                    let retry_status = retry_resp.status();
                    if !retry_status.is_success() {
                        let body_text =
                            tokio::time::timeout(Duration::from_secs(5), retry_resp.text())
                                .await
                                .unwrap_or(Ok(String::new()))
                                .unwrap_or_default();
                        return Err(LlmError::RequestFailed {
                            provider: "codex_chatgpt".to_string(),
                            reason: format!(
                                "HTTP {retry_status} from {url} (after token refresh): {body_text}"
                            ),
                        });
                    }

                    return Ok(retry_resp);
                } else {
                    tracing::warn!(
                        "Token refresh failed. Please re-authenticate with: codex --login"
                    );
                }
            }

            // No refresh token or refresh failed — return the 401 error
            // Drain the response body to release the connection
            let _ = resp.text().await;
            return Err(LlmError::AuthFailed {
                provider: "codex_chatgpt".to_string(),
            });
        }

        if !status.is_success() {
            // Read the error body with a timeout to avoid hanging
            let body_text = tokio::time::timeout(Duration::from_secs(5), resp.text())
                .await
                .unwrap_or(Ok(String::new()))
                .unwrap_or_default();
            return Err(LlmError::RequestFailed {
                provider: "codex_chatgpt".to_string(),
                reason: format!("HTTP {status} from {url}: {body_text}",),
            });
        }

        Ok(resp)
    }

    async fn send_request(&self, body: Value) -> Result<ResponsesResult, LlmError> {
        let resp = self.send_responses_request(&body).await?;
        Self::parse_sse_response_stream(resp, self.request_timeout).await
    }

    /// Low-level HTTP POST to the /responses endpoint.
    async fn send_http_request(
        client: &Client,
        url: &str,
        api_key: &SecretString,
        body: &Value,
        timeout: Duration,
    ) -> Result<reqwest::Response, LlmError> {
        client
            .post(url)
            .bearer_auth(api_key.expose_secret())
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| LlmError::RequestFailed {
                provider: "codex_chatgpt".to_string(),
                reason: format!("HTTP request failed: {e}"),
            })
    }

    async fn parse_sse_response_stream(
        resp: reqwest::Response,
        idle_timeout: Duration,
    ) -> Result<ResponsesResult, LlmError> {
        let stream = resp
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| e.to_string()));
        Self::parse_sse_stream_inner(stream, idle_timeout, None).await
    }

    async fn parse_sse_response_stream_with_events(
        resp: reqwest::Response,
        idle_timeout: Duration,
        event_tx: &mpsc::Sender<Result<LlmStreamEvent, LlmError>>,
    ) -> Result<ResponsesResult, LlmError> {
        let stream = resp
            .bytes_stream()
            .map(|chunk| chunk.map_err(|e| e.to_string()));
        Self::parse_sse_stream_inner(stream, idle_timeout, Some(event_tx)).await
    }

    #[cfg(test)]
    async fn parse_sse_stream<S>(
        stream: S,
        idle_timeout: Duration,
    ) -> Result<ResponsesResult, LlmError>
    where
        S: Stream<Item = Result<bytes::Bytes, String>> + Unpin,
    {
        Self::parse_sse_stream_inner(stream, idle_timeout, None).await
    }

    async fn parse_sse_stream_inner<S>(
        stream: S,
        idle_timeout: Duration,
        event_tx: Option<&mpsc::Sender<Result<LlmStreamEvent, LlmError>>>,
    ) -> Result<ResponsesResult, LlmError>
    where
        S: Stream<Item = Result<bytes::Bytes, String>> + Unpin,
    {
        let mut result = ResponsesResult::default();
        let mut stream = stream.eventsource();

        loop {
            match tokio::time::timeout(idle_timeout, stream.next()).await {
                Ok(Some(Ok(event))) => {
                    let data = event.data.trim();
                    if data.is_empty() {
                        continue;
                    }

                    let parsed: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let r5_start = result.r5_events.len();
                    let completed =
                        Self::handle_sse_event(&mut result, event.event.as_str(), &parsed);
                    if let Some(event_tx) = event_tx {
                        let new_events = result.r5_events.drain(r5_start..).collect::<Vec<_>>();
                        for event in new_events {
                            if event_tx.send(Ok(event)).await.is_err() {
                                return Ok(result);
                            }
                        }
                    }
                    if completed {
                        return Ok(result);
                    }
                }
                Ok(Some(Err(e))) => {
                    return Err(LlmError::RequestFailed {
                        provider: "codex_chatgpt".to_string(),
                        reason: format!("Failed to read SSE stream: {e}"),
                    });
                }
                Ok(None) => return Ok(result),
                Err(_) => {
                    return Err(LlmError::RequestFailed {
                        provider: "codex_chatgpt".to_string(),
                        reason: format!(
                            "Timed out waiting for SSE event after {}s",
                            idle_timeout.as_secs()
                        ),
                    });
                }
            }
        }
    }

    /// Parse SSE events from the response text.
    #[cfg(test)]
    fn parse_sse_response(sse_text: &str) -> Result<ResponsesResult, LlmError> {
        let mut result = ResponsesResult::default();
        let mut current_event_type = String::new();

        for line in sse_text.lines() {
            if let Some(event) = line.strip_prefix("event: ") {
                current_event_type = event.trim().to_string();
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }

                let parsed: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if Self::handle_sse_event(&mut result, current_event_type.as_str(), &parsed) {
                    return Ok(result);
                }
            }
        }

        Ok(result)
    }

    fn handle_sse_event(result: &mut ResponsesResult, event_type: &str, parsed: &Value) -> bool {
        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = parsed.get("delta").and_then(|d| d.as_str()) {
                    result.text.push_str(delta);
                }
            }
            "response.reasoning_text.delta" | "response.reasoning_content.delta" => {
                if let Some(delta) = parsed.get("delta").and_then(|d| d.as_str()) {
                    result
                        .r5_events
                        .push(LlmStreamEvent::ReasoningRawTextDelta {
                            item_id: parsed
                                .get("item_id")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            content_index: parsed
                                .get("content_index")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0),
                            delta: delta.to_string(),
                        });
                }
            }
            "response.output_item.added" => {
                // Capture function call metadata when the item is first added.
                // The item has: id (item_id), call_id, name, type.
                let item = parsed.get("item").unwrap_or(parsed);
                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                    let item_id = item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let call_id = item
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    result
                        .pending_tool_calls
                        .entry(item_id)
                        .or_insert_with(|| PendingToolCall {
                            call_id,
                            name,
                            arguments: String::new(),
                        });
                }
            }
            "response.output_item.done" | "response.output_item.completed" => {
                if let Some(item) = parsed.get("item") {
                    result
                        .r5_events
                        .push(LlmStreamEvent::RawResponseItemCompleted { item: item.clone() });
                }
            }
            "response.function_call_arguments.delta" => {
                // Delta events use `item_id` (not `call_id`)
                if let Some(item_id) = parsed.get("item_id").and_then(|v| v.as_str())
                    && let Some(entry) = result.pending_tool_calls.get_mut(item_id)
                    && let Some(delta) = parsed.get("delta").and_then(|d| d.as_str())
                {
                    entry.arguments.push_str(delta);
                }
            }
            "turn.plan.updated" => {
                // Upstream must provide a real turn-level plan event. Do not
                // synthesize plans from ordinary assistant text.
                let plan = parsed
                    .get("plan")
                    .and_then(parse_plan_steps)
                    .unwrap_or_default();
                if !plan.is_empty() {
                    result.r5_events.push(LlmStreamEvent::TurnPlanUpdated {
                        explanation: parsed
                            .get("explanation")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        plan,
                    });
                }
            }
            "turn.diff.updated" => {
                // Upstream must provide a real turn-level diff event. Do not
                // synthesize diffs from ordinary assistant text.
                if let Some(diff) = parsed.get("diff").and_then(|v| v.as_str()) {
                    if !diff.is_empty() {
                        result
                            .r5_events
                            .push(LlmStreamEvent::TurnDiffUpdated { diff: diff.into() });
                    }
                }
            }
            "response.completed" => {
                if let Some(response) = parsed.get("response")
                    && let Some(usage) = response.get("usage")
                {
                    result.input_tokens = usage
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    result.output_tokens = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                }
                return true;
            }
            _ => {}
        }

        false
    }

    /// Remove keys with empty-string values from a JSON object.
    ///
    /// gpt-5.2-codex fills optional tool parameters with `""` (e.g.
    /// `"timestamp": ""`). IronClaw's tool validation treats these as
    /// invalid "non-empty input expected". Stripping them makes the
    /// tool see only the actually-provided values.
    fn strip_empty_string_values(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let cleaned: serde_json::Map<String, Value> = map
                    .into_iter()
                    .filter(|(_, v)| !matches!(v, Value::String(s) if s.is_empty()))
                    .map(|(k, v)| (k, Self::strip_empty_string_values(v)))
                    .collect();
                Value::Object(cleaned)
            }
            other => other,
        }
    }

    fn tool_completion_from_responses_result(result: ResponsesResult) -> ToolCompletionResponse {
        let tool_calls: Vec<ToolCall> = result
            .pending_tool_calls
            .into_values()
            .map(|tc| {
                let args: Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_else(|_| json!(tc.arguments));
                // gpt-5.2-codex fills optional parameters with empty strings (e.g.
                // `"timestamp": ""`), which IronClaw's tool validation rejects.
                // Strip them so only actually-provided values reach the tool.
                let args = Self::strip_empty_string_values(args);
                ToolCall {
                    id: tc.call_id,
                    name: tc.name,
                    arguments: args,
                    reasoning: None,
                }
            })
            .collect();

        let finish_reason = if tool_calls.is_empty() {
            FinishReason::Stop
        } else {
            FinishReason::ToolUse
        };

        ToolCompletionResponse {
            content: if result.text.is_empty() {
                None
            } else {
                Some(result.text)
            },
            reasoning: None,
            tool_calls,
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
            finish_reason,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        }
    }
}

fn parse_plan_steps(value: &Value) -> Option<Vec<LlmPlanStep>> {
    let steps = value.as_array()?;
    Some(
        steps
            .iter()
            .filter_map(|step| {
                let text = step.get("step").and_then(|v| v.as_str())?;
                let status = match step.get("status").and_then(|v| v.as_str()) {
                    Some("pending") => LlmPlanStepStatus::Pending,
                    Some("inProgress") => LlmPlanStepStatus::InProgress,
                    Some("completed") => LlmPlanStepStatus::Completed,
                    _ => return None,
                };
                Some(LlmPlanStep {
                    step: text.to_string(),
                    status,
                })
            })
            .collect(),
    )
}

#[derive(Debug, Default)]
struct ResponsesResult {
    text: String,
    /// Keyed by item_id (the SSE item identifier, e.g. "fc_...").
    pending_tool_calls: std::collections::HashMap<String, PendingToolCall>,
    r5_events: Vec<LlmStreamEvent>,
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug)]
struct PendingToolCall {
    /// The call_id from the API (e.g. "call_..."), used to match results.
    call_id: String,
    name: String,
    arguments: String,
}

#[async_trait]
impl LlmProvider for CodexChatGptProvider {
    fn model_name(&self) -> &str {
        // Return resolved model if available, otherwise the configured name.
        self.resolved_model
            .get()
            .map(|s| s.as_str())
            .unwrap_or(&self.configured_model)
    }

    fn cost_per_token(&self) -> (Decimal, Decimal) {
        // ChatGPT backend doesn't expose per-token pricing
        (Decimal::ZERO, Decimal::ZERO)
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let model = self.resolve_model().await;
        let body = self.build_request_body(model, &request.messages, &[], None);
        let result = self.send_request(body).await?;

        Ok(CompletionResponse {
            content: result.text,
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
            finish_reason: FinishReason::Stop,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        })
    }

    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, LlmError> {
        let model = self.resolve_model().await;
        let body = self.build_request_body(
            model,
            &request.messages,
            &request.tools,
            request.tool_choice.as_deref(),
        );
        let result = self.send_request(body).await?;

        Ok(Self::tool_completion_from_responses_result(result))
    }

    async fn stream_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<LlmStream, LlmError> {
        let model = self.resolve_model().await;
        let body = self.build_request_body(
            model,
            &request.messages,
            &request.tools,
            request.tool_choice.as_deref(),
        );
        let resp = self.send_responses_request(&body).await?;
        let idle_timeout = self.request_timeout;
        let (event_tx, event_rx) =
            tokio::sync::mpsc::channel::<Result<LlmStreamEvent, LlmError>>(16);

        tokio::spawn(async move {
            match Self::parse_sse_response_stream_with_events(resp, idle_timeout, &event_tx).await {
                Ok(result) => {
                    let completed = Self::tool_completion_from_responses_result(result);
                    let _ = event_tx
                        .send(Ok(LlmStreamEvent::Completed(completed)))
                        .await;
                }
                Err(error) => {
                    let _ = event_tx.send(Err(error)).await;
                }
            }
        });

        Ok(LlmStream::new(event_rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_message_conversion_user() {
        let items = CodexChatGptProvider::message_to_input_items(&ChatMessage::user("hello"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "user");
        assert_eq!(items[0]["content"][0]["type"], "input_text");
        assert_eq!(items[0]["content"][0]["text"], "hello");
    }

    #[test]
    fn test_message_conversion_user_with_image() {
        use super::super::provider::ImageUrl;
        let parts = vec![
            ContentPart::Text {
                text: "What's in this image?".to_string(),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "data:image/png;base64,iVBOR...".to_string(),
                    detail: None,
                },
            },
        ];
        let msg = ChatMessage::user_with_parts("", parts);
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "user");
        let content = items[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "input_text");
        assert_eq!(content[0]["text"], "What's in this image?");
        assert_eq!(content[1]["type"], "input_image");
        assert_eq!(content[1]["image_url"], "data:image/png;base64,iVBOR...");
    }
    #[test]
    fn test_message_conversion_assistant() {
        let items = CodexChatGptProvider::message_to_input_items(&ChatMessage::assistant("hi"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["content"][0]["type"], "output_text");
    }

    #[test]
    fn test_message_conversion_tool_result() {
        let msg = ChatMessage::tool_result("call_1", "search", "result text");
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call_output");
        assert_eq!(items[0]["call_id"], "call_1");
        assert_eq!(items[0]["output"], "result text");
    }

    #[test]
    fn test_message_conversion_assistant_with_tool_calls() {
        let tc = ToolCall {
            id: "call_1".to_string(),
            name: "search".to_string(),
            arguments: json!({"query": "rust"}),
            reasoning: None,
        };
        let msg = ChatMessage::assistant_with_tool_calls(Some("thinking...".into()), vec![tc]);
        let items = CodexChatGptProvider::message_to_input_items(&msg);
        // Should produce: 1 text message + 1 function_call
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["type"], "message");
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["name"], "search");
        assert_eq!(items[1]["call_id"], "call_1");
    }

    #[test]
    fn test_build_request_extracts_system_as_instructions() {
        let provider = CodexChatGptProvider::new("https://example.com", "key", "gpt-4o");
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("hello"),
        ];
        let body = provider.build_request_body("gpt-4o", &messages, &[], None);
        assert_eq!(body["instructions"], "You are helpful.");
        // input should only contain the user message, not the system message
        assert_eq!(body["input"].as_array().unwrap().len(), 1);
        // store must be false for ChatGPT backend
        assert_eq!(body["store"], false);
    }

    #[test]
    fn test_parse_sse_text_response() {
        let sse = r#"event: response.output_text.delta
data: {"delta":"Hello"}

event: response.output_text.delta
data: {"delta":" world!"}

event: response.completed
data: {"response":{"usage":{"input_tokens":10,"output_tokens":5}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert_eq!(result.text, "Hello world!");
        assert_eq!(result.input_tokens, 10);
        assert_eq!(result.output_tokens, 5);
        assert!(result.pending_tool_calls.is_empty());
    }

    #[test]
    fn test_parse_sse_tool_call() {
        // Real API format: output_item.added has item.id (item_id) + item.call_id,
        // delta events use item_id (not call_id)
        let sse = r#"event: response.output_item.added
data: {"item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"search"}}

event: response.function_call_arguments.delta
data: {"item_id":"fc_1","delta":"{\"query\":"}

event: response.function_call_arguments.delta
data: {"item_id":"fc_1","delta":"\"rust\"}"}

event: response.completed
data: {"response":{"usage":{"input_tokens":20,"output_tokens":15}}}

"#;
        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.text.is_empty());
        assert_eq!(result.pending_tool_calls.len(), 1);
        let tc = result.pending_tool_calls.get("fc_1").unwrap();
        assert_eq!(tc.call_id, "call_1");
        assert_eq!(tc.name, "search");
        assert_eq!(tc.arguments, "{\"query\":\"rust\"}");
    }

    #[test]
    fn codex_chatgpt_fixture_emits_raw_response_item_completed() {
        let sse = r#"event: response.output_item.done
data: {"item":{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}

event: response.completed
data: {"response":{"usage":{"input_tokens":1,"output_tokens":1}}}

"#;

        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.r5_events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::RawResponseItemCompleted { item }
                    if item["id"] == "msg_1" && item["type"] == "message"
            )
        }));
    }

    #[test]
    fn codex_chatgpt_fixture_emits_reasoning_raw_text_delta() {
        let sse = r#"event: response.reasoning_text.delta
data: {"item_id":"rs_1","content_index":2,"delta":"raw step"}

event: response.completed
data: {"response":{"usage":{"input_tokens":1,"output_tokens":1}}}

"#;

        let result = CodexChatGptProvider::parse_sse_response(sse).unwrap();
        assert!(result.r5_events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::ReasoningRawTextDelta {
                    item_id: Some(item_id),
                    content_index: 2,
                    delta,
                } if item_id == "rs_1" && delta == "raw step"
            )
        }));
    }

    #[test]
    fn provider_fixture_emits_plan_or_diff_only_when_source_event_exists() {
        let text_only_sse = r#"event: response.output_text.delta
data: {"delta":"Plan: do not synthesize this into a turn plan\nDiff: not an upstream diff event"}

event: response.completed
data: {"response":{"usage":{"input_tokens":1,"output_tokens":1}}}

"#;

        let text_only_result = CodexChatGptProvider::parse_sse_response(text_only_sse).unwrap();
        assert!(text_only_result.r5_events.iter().all(|event| {
            !matches!(
                event,
                LlmStreamEvent::PlanDelta { .. }
                    | LlmStreamEvent::TurnPlanUpdated { .. }
                    | LlmStreamEvent::TurnDiffUpdated { .. }
            )
        }));

        let event_source_sse = r#"event: turn.plan.updated
data: {"explanation":"need two steps","plan":[{"step":"inspect","status":"completed"},{"step":"patch","status":"inProgress"}]}

event: turn.diff.updated
data: {"diff":"diff --git a/file b/file\n+added\n"}

event: response.completed
data: {"response":{"usage":{"input_tokens":1,"output_tokens":1}}}

"#;

        let event_source_result =
            CodexChatGptProvider::parse_sse_response(event_source_sse).unwrap();
        assert!(event_source_result.r5_events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::TurnPlanUpdated { explanation, plan }
                    if explanation.as_deref() == Some("need two steps") && plan.len() == 2
            )
        }));
        assert!(event_source_result.r5_events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::TurnDiffUpdated { diff }
                    if diff.contains("diff --git") && diff.contains("+added")
            )
        }));

        let empty_diff_sse = r#"event: turn.diff.updated
data: {"diff":""}

event: response.completed
data: {"response":{"usage":{"input_tokens":1,"output_tokens":1}}}

"#;

        let empty_diff_result = CodexChatGptProvider::parse_sse_response(empty_diff_sse).unwrap();
        assert!(
            empty_diff_result
                .r5_events
                .iter()
                .all(|event| !matches!(event, LlmStreamEvent::TurnDiffUpdated { .. }))
        );
    }

    #[tokio::test]
    async fn codex_chatgpt_stream_with_tools_emits_r5_events_from_http_responses_sse() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "models": [{"slug": "gpt-4o"}]
            })))
            .mount(&server)
            .await;

        let sse = r#"event: response.reasoning_text.delta
data: {"item_id":"rs_1","content_index":2,"delta":"raw step"}

event: response.output_item.done
data: {"item":{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}

event: turn.plan.updated
data: {"explanation":"provider event","plan":[{"step":"inspect","status":"completed"},{"step":"patch","status":"inProgress"}]}

event: turn.diff.updated
data: {"diff":"diff --git a/file b/file\n+added\n"}

event: response.completed
data: {"response":{"usage":{"input_tokens":3,"output_tokens":2}}}

"#;
        Mock::given(method("POST"))
            .and(path("/responses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse),
            )
            .mount(&server)
            .await;

        let provider = CodexChatGptProvider::new(&server.uri(), "key", "gpt-4o");
        let request = ToolCompletionRequest::new(vec![ChatMessage::user("hello")], vec![]);
        let mut stream = provider.stream_with_tools(request).await.unwrap();
        let mut events = Vec::new();

        while let Some(event) = stream.next().await {
            let event = event.unwrap();
            let is_completed = matches!(event, LlmStreamEvent::Completed(_));
            events.push(event);
            if is_completed {
                break;
            }
        }

        assert!(events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::ReasoningRawTextDelta {
                    item_id: Some(item_id),
                    content_index: 2,
                    delta,
                } if item_id == "rs_1" && delta == "raw step"
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::RawResponseItemCompleted { item }
                    if item["id"] == "msg_1" && item["type"] == "message"
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::TurnPlanUpdated { explanation, plan }
                    if explanation.as_deref() == Some("provider event") && plan.len() == 2
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::TurnDiffUpdated { diff }
                    if diff.contains("diff --git") && diff.contains("+added")
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                LlmStreamEvent::Completed(response)
                    if response.input_tokens == 3 && response.output_tokens == 2
            )
        }));
    }

    #[tokio::test]
    async fn codex_chatgpt_stream_with_tools_yields_r5_events_before_response_completed() {
        let (base_url, release_completed) = spawn_delayed_responses_fixture_server();
        let provider = CodexChatGptProvider::new(&base_url, "key", "gpt-4o");
        let request = ToolCompletionRequest::new(vec![ChatMessage::user("hello")], vec![]);

        let stream_result =
            tokio::time::timeout(Duration::from_secs(1), provider.stream_with_tools(request)).await;
        let mut stream = stream_result
            .expect("stream_with_tools should return before response.completed")
            .expect("stream should be created");

        let first_event = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("first stream event should arrive before response.completed")
            .expect("stream should yield an event")
            .expect("first stream event should be ok");
        assert!(matches!(
            first_event,
            LlmStreamEvent::TurnPlanUpdated { explanation, plan }
                if explanation.as_deref() == Some("delayed plan")
                    && plan.len() == 1
                    && plan[0].step == "inspect"
        ));

        release_completed
            .send(())
            .expect("fixture should still be waiting to release completion");
        let completed = tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(event) = stream.next().await {
                let event = event?;
                if matches!(event, LlmStreamEvent::Completed(_)) {
                    return Ok::<(), LlmError>(());
                }
            }
            Err(LlmError::InvalidResponse {
                provider: "codex_chatgpt".to_string(),
                reason: "stream ended before completion".to_string(),
            })
        })
        .await
        .expect("completion should arrive after fixture release");
        completed.expect("completion event should be ok");
    }

    #[tokio::test]
    async fn test_parse_sse_stream_response() {
        let stream = stream::iter(vec![
            Ok(Bytes::from_static(
                b"event: response.output_text.delta\ndata: {\"delta\":\"Hello\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"event: response.output_text.delta\ndata: {\"delta\":\" world\"}\n\n",
            )),
            Ok(Bytes::from_static(
                b"event: response.completed\ndata: {\"response\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\n\n",
            )),
        ]);

        let result = CodexChatGptProvider::parse_sse_stream(stream, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.input_tokens, 3);
        assert_eq!(result.output_tokens, 2);
    }

    #[test]
    fn test_strip_empty_string_values() {
        let input = json!({
            "format": "%Y-%m-%d",
            "operation": "now",
            "timestamp": "",
            "timestamp2": "",
        });
        let cleaned = CodexChatGptProvider::strip_empty_string_values(input);
        assert_eq!(cleaned, json!({"format": "%Y-%m-%d", "operation": "now"}));
    }

    fn spawn_delayed_responses_fixture_server() -> (String, std::sync::mpsc::Sender<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("fixture server should bind localhost");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            for (expected_prefix, content_type, body) in [
                (
                    "GET /models",
                    "application/json",
                    r#"{"models":[{"slug":"gpt-4o"}]}"#.to_string(),
                ),
                (
                    "POST /responses",
                    "text/event-stream",
                    r#"event: turn.plan.updated
data: {"explanation":"delayed plan","plan":[{"step":"inspect","status":"inProgress"}]}

"#
                    .to_string(),
                ),
            ] {
                let (mut stream, _) = listener.accept().expect("fixture should accept request");
                let request = read_fixture_http_request(&mut stream);
                let request_line = request.lines().next().unwrap_or_default();
                assert!(
                    request_line.starts_with(expected_prefix),
                    "expected request line prefix {expected_prefix:?}, got {request_line:?}"
                );

                if expected_prefix == "POST /responses" {
                    write_fixture_http_response_headers(&mut stream, content_type);
                    use std::io::Write as _;
                    stream
                        .write_all(body.as_bytes())
                        .expect("fixture should write first SSE event");
                    stream
                        .flush()
                        .expect("fixture should flush first SSE event");
                    release_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("test should release response.completed");
                    stream
                        .write_all(
                            b"event: response.completed\n\
data: {\"response\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":2}}}\n\n",
                        )
                        .expect("fixture should write completion SSE event");
                    stream
                        .flush()
                        .expect("fixture should flush completion SSE event");
                } else {
                    write_fixture_http_response(&mut stream, content_type, &body);
                }
            }
        });

        (base_url, release_tx)
    }

    fn read_fixture_http_request(stream: &mut std::net::TcpStream) -> String {
        use std::io::Read as _;

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("fixture request stream should support read timeout");
        let mut buffer = Vec::new();
        let mut chunk = [0; 1024];
        loop {
            let read = stream
                .read(&mut chunk)
                .expect("fixture should read request bytes");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&buffer).into_owned()
    }

    fn write_fixture_http_response(
        stream: &mut std::net::TcpStream,
        content_type: &str,
        body: &str,
    ) {
        write_fixture_http_response_headers(stream, content_type);
        use std::io::Write as _;
        stream
            .write_all(body.as_bytes())
            .expect("fixture should write response body");
        stream.flush().expect("fixture should flush response body");
    }

    fn write_fixture_http_response_headers(stream: &mut std::net::TcpStream, content_type: &str) {
        use std::io::Write as _;

        let headers =
            format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(headers.as_bytes())
            .expect("fixture should write response headers");
        stream
            .flush()
            .expect("fixture should flush response headers");
    }
}
