//! TauriChannel — 桥接 IronClaw Agent 和 Tauri 前端的 Channel 实现。
//!
//! 实现 `ironclaw::channels::Channel` trait，将 Agent 的输出直接转为
//! Tauri IPC 事件推送到前端。无需 HTTP/SSE/WebSocket，进程内零开销通信。
//!
//! # 架构
//!
//! 统一使用 `chat-stream` 通道，发送 `VercelUIStream` 事件：
//! - 工具事件 → `ToolInputStart/Available`, `ToolOutputAvailable/Error`
//! - 文本流 → `TextDelta`
//! - 思考链 → `ReasoningDelta`
//! - 非标准事件 → `DataCustom { data: {"type": "...", ...} }`
//!
//! ```text
//! 前端 invoke("send_chat_message")
//!   → TauriChannel.incoming_tx.send(IncomingMessage)
//!   → Agent 处理
//!   → TauriChannel.respond() / send_status()
//!   → app_handle.emit("chat-stream", VercelUIStream)
//!   → 前端 listen("chat-stream")
//! ```

use async_trait::async_trait;
use ironclaw::channels::{Channel, IncomingMessage, MessageStream, OutgoingResponse, StatusUpdate};
use ironclaw::error::ChannelError;
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};

use crate::conversation_tracker::ConversationTracker;
use crate::vercel_ui_protocol::VercelUIStream;

// ---------------------------------------------------------------------------
// TauriJobEventSink — Worker 事件广播到 Tauri IPC
// ---------------------------------------------------------------------------

/// 实现 `JobEventSink` trait，将 Worker 的 job 事件推送到前端。
///
/// 只关注 `"result"` 和 `"status"` 事件类型（任务完成/状态变更），
/// 其他事件（tool_use、reasoning 等）在桌面客户端不需要实时推送。
pub struct TauriJobEventSink {
    app_handle: AppHandle,
}

impl TauriJobEventSink {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl ironclaw::worker::JobEventSink for TauriJobEventSink {
    fn send_job_event(&self, job_id: uuid::Uuid, event_type: &str, data: &serde_json::Value) {
        match event_type {
            "result" | "status" => {
                let status = data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .or_else(|| data.get("message").and_then(|v| v.as_str()))
                    .unwrap_or("unknown");
                let title = data
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let event = VercelUIStream::DataCustom {
                    id: None,
                    data: json!({
                        "type": "job_status",
                        "job_id": job_id.to_string(),
                        "title": title,
                        "status": status,
                    }),
                };
                let _ = self.app_handle.emit("chat-stream", &event);
            }
            _ => {} // tool_use、reasoning 等不推送
        }
    }
}

// ---------------------------------------------------------------------------
// TauriChannel
// ---------------------------------------------------------------------------

/// TauriChannel — 实现 IronClaw `Channel` trait 的 Tauri IPC 桥接。
///
/// 生命周期：
/// 1. `new()` 创建实例，同时生成 `mpsc::Sender` 供 Tauri Command 注入消息。
/// 2. `start()` 被 `ChannelManager::start_all()` 调用，返回消息流。
/// 3. Agent 处理消息后通过 `respond()` / `send_status()` 推送事件到前端。
pub struct TauriChannel {
    app_handle: AppHandle,
    incoming_tx: mpsc::Sender<IncomingMessage>,
    /// `start()` 只能调用一次，取走 receiver。
    incoming_rx: Mutex<Option<mpsc::Receiver<IncomingMessage>>>,
    /// 对话追踪器，用于收集 Token 消耗并在对话结束时上报。
    pub conversation_tracker: Option<Arc<ConversationTracker>>,
}

impl TauriChannel {
    /// 创建新的 TauriChannel。
    ///
    /// `buffer_size` 控制消息队列深度，默认 64 足以应对正常交互。
    pub fn new(app_handle: AppHandle) -> Self {
        Self::with_buffer_size(app_handle, 64)
    }

    /// 创建指定缓冲区大小的 TauriChannel。
    pub fn with_buffer_size(app_handle: AppHandle, buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self {
            app_handle,
            incoming_tx: tx,
            incoming_rx: Mutex::new(Some(rx)),
            conversation_tracker: None,
        }
    }

    /// 注入对话追踪器（引擎启动后调用）。
    pub fn set_conversation_tracker(&mut self, tracker: Arc<ConversationTracker>) {
        self.conversation_tracker = Some(tracker);
    }

    /// 获取消息发送端。
    ///
    /// Tauri Command 通过此 sender 将用户消息注入 Agent 消息循环。
    pub fn sender(&self) -> mpsc::Sender<IncomingMessage> {
        self.incoming_tx.clone()
    }

    /// 向前端发送 Vercel AI protocol 事件（`chat-stream` 通道）。
    fn emit_stream(&self, event: &VercelUIStream) -> Result<(), ChannelError> {
        self.app_handle
            .emit("chat-stream", event)
            .map_err(|e| ChannelError::SendFailed {
                name: "tauri".into(),
                reason: e.to_string(),
            })
    }

    /// 将 `StatusUpdate` 映射为 `VercelUIStream` 事件并发送。
    ///
    /// - 工具/文本/推理事件 → 原生 Vercel AI protocol 类型
    /// - 其他事件 → `DataCustom { data: {"type": "...", ...} }`
    pub(crate) fn emit_status_stream(
        &self,
        status: &StatusUpdate,
        metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        for event in map_status_to_stream(status, metadata) {
            self.emit_stream(&event)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 纯映射函数 — StatusUpdate → Vec<VercelUIStream>
// ---------------------------------------------------------------------------

/// 将 `StatusUpdate` + metadata 映射为 `VercelUIStream` 事件列表（纯函数）。
///
/// 返回 0~2 个事件。测试可直接调用此函数验证映射逻辑，无需 `AppHandle`。
pub(crate) fn map_status_to_stream(
    status: &StatusUpdate,
    metadata: &serde_json::Value,
) -> Vec<VercelUIStream> {
    fn tool_call_id(metadata: &serde_json::Value) -> String {
        metadata
            .get("_tool_call_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    match status {
        StatusUpdate::Thinking(msg) => vec![VercelUIStream::ReasoningDelta {
            id: String::new(),
            delta: msg.clone(),
            provider_metadata: None,
        }],

        StatusUpdate::StreamChunk(content) => vec![VercelUIStream::TextDelta {
            id: String::new(),
            delta: content.clone(),
            provider_metadata: None,
        }],

        StatusUpdate::ToolStarted { name } => {
            let tcid = tool_call_id(metadata);
            if tcid.is_empty() {
                return vec![];
            }
            let tool_args = metadata
                .get("_tool_arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            vec![
                VercelUIStream::ToolInputStart {
                    tool_call_id: tcid.clone(),
                    tool_name: name.clone(),
                    provider_executed: Some(true),
                    provider_metadata: None,
                },
                VercelUIStream::ToolInputAvailable {
                    tool_call_id: tcid,
                    tool_name: name.clone(),
                    input: tool_args,
                    provider_executed: Some(true),
                    provider_metadata: None,
                },
            ]
        }

        StatusUpdate::ToolResult { preview, .. } => {
            let tcid = tool_call_id(metadata);
            if tcid.is_empty() {
                return vec![];
            }
            vec![VercelUIStream::ToolOutputAvailable {
                tool_call_id: tcid,
                output: serde_json::Value::String(preview.clone()),
                provider_executed: Some(true),
            }]
        }

        StatusUpdate::ToolCompleted {
            name,
            success: false,
            error,
            ..
        } => {
            let tcid = tool_call_id(metadata);
            if tcid.is_empty() {
                return vec![VercelUIStream::DataCustom {
                    id: None,
                    data: json!({
                        "type": "tool_completed",
                        "name": name,
                        "success": false,
                        "error": error,
                    }),
                }];
            }
            vec![VercelUIStream::ToolOutputError {
                tool_call_id: tcid,
                error_text: error
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string()),
                provider_executed: Some(true),
            }]
        }

        StatusUpdate::ToolCompleted {
            name,
            success: true,
            ..
        } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "tool_completed",
                "name": name,
                "success": true,
            }),
        }],

        StatusUpdate::Status(msg) => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({ "type": "status", "message": msg, "level": "info" }),
        }],

        StatusUpdate::JobStarted {
            job_id, title, ..
        } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "job_status",
                "job_id": job_id,
                "title": title,
                "status": "in_progress",
            }),
        }],

        StatusUpdate::ApprovalNeeded {
            request_id,
            tool_name,
            description,
            ..
        } => {
            let thread_id = metadata
                .get("notify_thread_id")
                .and_then(|v| v.as_str())
                .or_else(|| metadata.get("thread_id").and_then(|v| v.as_str()))
                .unwrap_or_default();
            tracing::info!(
                tool_name = %tool_name,
                request_id = %request_id,
                thread_id = %thread_id,
                "Emitting approval_needed event to frontend"
            );
            // 双发：
            //   - 旧 `DataCustom` 事件：兼容 TauriRuntimeProvider 的 useApprovalState
            //   - 新 `ToolInputAvailable` 事件：对接 assistant-ui `approval_request` ToolUI
            //     （request_id 同时作为 toolCallId，SDK 按 id 自动挂载到正确 branch）
            //
            // 注意：parameters 字段**不**透传给前端，防止敏感信息泄露（与旧 DataCustom 对齐）。
            vec![
                VercelUIStream::DataCustom {
                    id: None,
                    data: json!({
                        "type": "approval_needed",
                        "thread_id": thread_id,
                        "request_id": request_id,
                        "tool_name": tool_name,
                        "description": description,
                    }),
                },
                VercelUIStream::ToolInputAvailable {
                    tool_call_id: request_id.to_string(),
                    tool_name: "approval_request".to_string(),
                    input: json!({
                        "request_id": request_id,
                        "tool_name": tool_name,
                        "description": description,
                    }),
                    provider_executed: None,
                    provider_metadata: None,
                },
            ]
        }

        StatusUpdate::AuthRequired {
            extension_name,
            instructions,
            ..
        } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "status",
                "message": format!(
                    "Extension '{}' requires authentication{}",
                    extension_name,
                    instructions.as_ref().map(|i| format!(": {}", i)).unwrap_or_default()
                ),
                "level": "warn",
            }),
        }],

        StatusUpdate::AuthCompleted {
            extension_name,
            success,
            message,
        } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "status",
                "message": format!(
                    "Extension '{}' auth {}: {}",
                    extension_name,
                    if *success { "succeeded" } else { "failed" },
                    message
                ),
                "level": if *success { "info" } else { "error" },
            }),
        }],

        StatusUpdate::ImageGenerated { data_url, path } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "image_generated",
                "data_url": data_url,
                "path": path,
            }),
        }],

        StatusUpdate::Suggestions { suggestions } => vec![VercelUIStream::DataCustom {
            id: None,
            data: json!({ "type": "suggestions", "suggestions": suggestions }),
        }],

        // ReasoningUpdate 和 TurnCost 不推送到前端
        StatusUpdate::ReasoningUpdate { .. } | StatusUpdate::TurnCost { .. } => vec![],
    }
}

// ---------------------------------------------------------------------------
// Channel trait 实现
// ---------------------------------------------------------------------------

#[async_trait]
impl Channel for TauriChannel {
    fn name(&self) -> &str {
        "tauri"
    }

    async fn start(&self) -> Result<MessageStream, ChannelError> {
        let rx =
            self.incoming_rx
                .lock()
                .await
                .take()
                .ok_or_else(|| ChannelError::StartupFailed {
                    name: "tauri".into(),
                    reason: "Channel already started (start() called twice)".into(),
                })?;

        // 通知前端引擎就绪
        let _ = self.emit_stream(&VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "connection_status",
                "connected": true,
                "message": "IronClaw engine ready",
            }),
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn respond(
        &self,
        msg: &IncomingMessage,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        let thread_id = msg.thread_id.clone().unwrap_or_default();

        // 记录 assistant 消息到对话追踪器（Token 数在后续 TokenUsage 事件中更新）
        if let Some(tracker) = &self.conversation_tracker {
            tracker.record_assistant_message(&thread_id, &response.content, None, 0, 0);
        }

        self.emit_stream(&VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": msg.id.to_string(),
                "content": response.content,
                "thread_id": thread_id,
                "source": "chat",
            }),
        })?;
        self.emit_stream(&VercelUIStream::Finish {
            id: msg.id.to_string(),
        })
    }

    async fn send_status(
        &self,
        status: StatusUpdate,
        metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        // routine_triggered → DataCustom
        if metadata
            .get("routine_triggered")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return self.emit_stream(&VercelUIStream::DataCustom {
                id: None,
                data: json!({
                    "type": "routine_triggered",
                    "thread_id": metadata.get("thread_id").and_then(|v| v.as_str()).unwrap_or_default(),
                    "fired": metadata.get("fired").and_then(|v| v.as_u64()).unwrap_or(1),
                }),
            });
        }

        // TurnCost → 只更新对话追踪器，不推送到前端
        if let StatusUpdate::TurnCost {
            input_tokens,
            output_tokens,
            ..
        } = &status
        {
            if let Some(tracker) = &self.conversation_tracker {
                let thread_id = metadata
                    .get("notify_thread_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                tracker.update_last_assistant_tokens(
                    thread_id,
                    "",
                    (*input_tokens).min(i32::MAX as u64) as i32,
                    (*output_tokens).min(i32::MAX as u64) as i32,
                );
            }
            return Ok(());
        }

        self.emit_status_stream(&status, metadata)
    }

    async fn broadcast(
        &self,
        _user_id: &str,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        self.emit_stream(&VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": uuid::Uuid::new_v4().to_string(),
                "content": response.content,
                "thread_id": response.thread_id.unwrap_or_default(),
                "source": "routine",
            }),
        })
    }

    async fn health_check(&self) -> Result<(), ChannelError> {
        // 进程内通信，始终健康。
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), ChannelError> {
        let _ = self.emit_stream(&VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "connection_status",
                "connected": false,
                "message": "IronClaw engine shutting down",
            }),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a default metadata value (empty JSON object).
    fn empty_meta() -> serde_json::Value {
        json!({})
    }

    /// Helper: build metadata with _tool_call_id injected.
    fn tool_meta(tool_call_id: &str) -> serde_json::Value {
        json!({ "_tool_call_id": tool_call_id, "_tool_arguments": {"foo": "bar"} })
    }

    /// 验证所有 StatusUpdate 变体都能正确映射为 VercelUIStream 且序列化不 panic。
    #[test]
    fn test_status_to_stream_covers_all_variants() {
        let cases: Vec<(StatusUpdate, serde_json::Value)> = vec![
            (StatusUpdate::Thinking("processing...".into()), empty_meta()),
            (StatusUpdate::ToolStarted { name: "shell".into() }, tool_meta("tc-1")),
            (StatusUpdate::ToolCompleted { name: "shell".into(), success: true, error: None, parameters: None }, tool_meta("tc-1")),
            (StatusUpdate::ToolCompleted { name: "shell".into(), success: false, error: Some("fail".into()), parameters: None }, tool_meta("tc-2")),
            (StatusUpdate::ToolResult { name: "shell".into(), preview: "output...".into() }, tool_meta("tc-1")),
            (StatusUpdate::StreamChunk("hello ".into()), empty_meta()),
            (StatusUpdate::Status("ready".into()), empty_meta()),
            (StatusUpdate::JobStarted { job_id: "j-1".into(), title: "Build".into(), browse_url: "http://localhost".into() }, empty_meta()),
            (StatusUpdate::ApprovalNeeded { request_id: "r-1".into(), tool_name: "rm".into(), description: "delete file".into(), parameters: json!({}), allow_always: true }, empty_meta()),
            (StatusUpdate::AuthRequired { extension_name: "github".into(), instructions: Some("click link".into()), auth_url: None, setup_url: None }, empty_meta()),
            (StatusUpdate::AuthCompleted { extension_name: "github".into(), success: true, message: "ok".into() }, empty_meta()),
            (StatusUpdate::ImageGenerated { data_url: "data:image/png;base64,abc".into(), path: Some("/tmp/img.png".into()) }, empty_meta()),
            (StatusUpdate::Suggestions { suggestions: vec!["try this".into()] }, empty_meta()),
            (StatusUpdate::ReasoningUpdate { narrative: "Choosing search tool".into(), decisions: vec![] }, empty_meta()),
            (StatusUpdate::TurnCost { input_tokens: 100, output_tokens: 50, cost_usd: "$0.0010".into() }, empty_meta()),
        ];

        for (status, meta) in &cases {
            let events = map_status_to_stream(&status, &meta);
            // 每个事件都应该能成功序列化
            for event in &events {
                serde_json::to_value(event).expect("should serialize");
            }
        }
    }

    /// 验证 DataCustom response 事件格式与前端 TypeScript 类型兼容。
    #[test]
    fn test_response_event_serialization() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "response",
                "message_id": "msg-1",
                "content": "Hello",
                "thread_id": "t-1",
                "source": "chat",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["data"]["type"], "response");
        assert_eq!(json["data"]["message_id"], "msg-1");
        assert_eq!(json["data"]["content"], "Hello");
        assert_eq!(json["data"]["thread_id"], "t-1");
        assert_eq!(json["data"]["source"], "chat");
    }

    /// 验证 reasoning-delta 事件格式。
    #[test]
    fn test_thinking_maps_to_reasoning_delta() {
        let events = map_status_to_stream(
            &StatusUpdate::Thinking("analyzing...".into()),
            &empty_meta(),
        );
        assert_eq!(events.len(), 1);
        let json = serde_json::to_value(&events[0]).expect("should serialize");
        assert_eq!(json["type"], "reasoning-delta");
        assert_eq!(json["delta"], "analyzing...");
    }

    /// 验证 error 事件使用 Vercel protocol Error 类型。
    #[test]
    fn test_error_event_format() {
        let event = VercelUIStream::Error {
            error_text: "timeout".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "error");
        assert_eq!(json["errorText"], "timeout");
    }

    /// 验证 connection_status 通过 DataCustom 发送。
    #[test]
    fn test_connection_status_as_data_custom() {
        let event = VercelUIStream::DataCustom {
            id: None,
            data: json!({
                "type": "connection_status",
                "connected": true,
                "message": "ready",
            }),
        };
        let json: serde_json::Value = serde_json::to_value(&event).expect("should serialize");
        assert_eq!(json["type"], "data-custom");
        assert_eq!(json["data"]["type"], "connection_status");
        assert_eq!(json["data"]["connected"], true);
    }

    /// 安全测试：验证 ToolCompleted 失败时映射结果不包含 parameters。
    #[test]
    fn test_security_tool_completed_error_no_sensitive_leak() {
        let status = StatusUpdate::ToolCompleted {
            name: "secret_save".into(),
            success: false,
            error: Some("db connection failed".into()),
            parameters: Some(r#"{"value": "[REDACTED]"}"#.into()),
        };
        let events = map_status_to_stream(&status, &tool_meta("tc-1"));
        assert_eq!(events.len(), 1);
        let json = serde_json::to_string(&events[0]).expect("should serialize");
        assert!(!json.contains("REDACTED"), "parameters should not be in emitted event");
    }

    /// 契约测试：JobStarted → data-custom job_status 事件。
    #[test]
    fn test_contract_job_started_maps_to_job_status() {
        let status = StatusUpdate::JobStarted {
            job_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            title: "分析代码库".into(),
            browse_url: "http://localhost".into(),
        };
        let events = map_status_to_stream(&status, &empty_meta());
        assert_eq!(events.len(), 1);
        let json = serde_json::to_value(&events[0]).expect("should serialize");
        assert_eq!(json["data"]["type"], "job_status");
        assert_eq!(json["data"]["job_id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["data"]["title"], "分析代码库");
        assert_eq!(json["data"]["status"], "in_progress");
    }

    /// 安全审计：job_status 事件不应泄露 user_id 或 browse_url。
    #[test]
    fn test_audit_job_status_no_sensitive_fields() {
        let status = StatusUpdate::JobStarted {
            job_id: "job-001".into(),
            title: "任务标题".into(),
            browse_url: "http://localhost/secret".into(),
        };
        let events = map_status_to_stream(&status, &empty_meta());
        assert_eq!(events.len(), 1);
        let json_str = serde_json::to_string(&events[0]).expect("should serialize");
        assert!(!json_str.contains("user_id"), "user_id should not be exposed");
        assert!(!json_str.contains("browse_url"), "browse_url should not be forwarded");
        assert!(!json_str.contains("secret"), "browse_url value should not leak");
    }

    /// 验证 ToolStarted 无 tool_call_id 时返回空列表（不生成空 ID 的事件）。
    #[test]
    fn test_tool_started_without_tool_call_id_is_noop() {
        let status = StatusUpdate::ToolStarted {
            name: "shell".into(),
        };
        let events = map_status_to_stream(&status, &empty_meta());
        assert!(events.is_empty(), "ToolStarted without tool_call_id should produce no events");
    }
}
