//! TauriChannel — 桥接 IronClaw Agent 和 Tauri 前端的 Channel 实现。
//!
//! 实现 `ironclaw::channels::Channel` trait，将 Agent 的输出直接转为
//! Tauri IPC 事件推送到前端。无需 HTTP/SSE/WebSocket，进程内零开销通信。
//!
//! # 架构
//!
//! ```text
//! 前端 invoke("send_chat_message")
//!   → TauriChannel.incoming_tx.send(IncomingMessage)
//!   → Agent 处理
//!   → TauriChannel.respond() / send_status()
//!   → app_handle.emit("chat-event", ChatEvent)
//!   → 前端 listen("chat-event")
//! ```

use async_trait::async_trait;
use ironclaw::channels::{
    Channel, IncomingMessage, MessageStream, OutgoingResponse, StatusUpdate,
};
use ironclaw::error::ChannelError;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};

use crate::conversation_tracker::ConversationTracker;

// ---------------------------------------------------------------------------
// ChatEvent — 前端接收的聊天事件
// ---------------------------------------------------------------------------

/// 前端接收的聊天事件。
///
/// 与现有前端 `useAiChatTauri.ts` 中的 `ChatEvent` 类型完全兼容，
/// 确保前端代码零修改。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    /// AI 回复消息。
    #[serde(rename = "response")]
    Response {
        message_id: String,
        content: String,
        thread_id: String,
        /// 消息来源，用于前端区分普通聊天和定时任务通知。
        /// "chat" = 普通聊天回复，"routine" = 定时任务通知。
        #[serde(default)]
        source: String,
    },
    /// Agent 正在思考。
    #[serde(rename = "thinking")]
    Thinking { message: String },
    /// 通用状态更新。
    #[serde(rename = "status")]
    Status { message: String, level: String },
    /// 错误事件。
    #[serde(rename = "error")]
    Error {
        message: String,
        code: Option<String>,
    },
    /// 连接状态变更。
    #[serde(rename = "connection_status")]
    ConnectionStatus { connected: bool, message: String },
    // === 扩展事件（前端可按需处理）===
    /// 工具开始执行。
    #[serde(rename = "tool_started")]
    ToolStarted { name: String },
    /// 工具执行完成。
    #[serde(rename = "tool_completed")]
    ToolCompleted {
        name: String,
        success: bool,
        error: Option<String>,
    },
    /// 流式文本片段。
    #[serde(rename = "stream_chunk")]
    StreamChunk { content: String },
    /// 工具需要用户审批。
    #[serde(rename = "approval_needed")]
    ApprovalNeeded {
        request_id: String,
        tool_name: String,
        description: String,
    },
    /// 图片已生成。
    #[serde(rename = "image_generated")]
    ImageGenerated {
        data_url: String,
        path: Option<String>,
    },
    /// 建议的后续消息。
    #[serde(rename = "suggestions")]
    Suggestions { suggestions: Vec<String> },
    /// 后台任务状态变更（job 创建/完成时推送）。
    ///
    /// 前端用于实时更新任务列表和头部运行中计数，无需轮询。
    #[serde(rename = "job_status")]
    JobStatus {
        job_id: String,
        title: String,
        /// "in_progress" | "completed" | "failed"
        status: String,
    },
    /// 本轮对话激活的技能列表（desktop-client 侧检测，不依赖 ironclaw 事件）。
    ///
    /// 在 `send_chat_message` 中通过 `prefilter_skills` 本地匹配后发出，
    /// 供前端在 AI 回复前展示"正在使用技能 X"的提示。
    #[serde(rename = "skills_activated")]
    SkillsActivated {
        /// 激活的技能名称列表（按匹配分数排序）。
        skills: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// TauriJobEventSink — Worker 事件广播到 Tauri IPC
// ---------------------------------------------------------------------------

/// 实现 `JobEventSink` trait，将 Worker 的 job 事件转为 `ChatEvent::JobStatus` 推送到前端。
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
        let event = match event_type {
            "result" | "status" => {
                let status = data
                    .get("status")
                    .and_then(|v| v.as_str())
                    .or_else(|| data.get("message").and_then(|v| v.as_str()))
                    .unwrap_or("unknown")
                    .to_string();
                let title = data
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                ChatEvent::JobStatus {
                    job_id: job_id.to_string(),
                    title,
                    status,
                }
            }
            _ => return, // tool_use、reasoning 等不推送
        };
        let _ = self.app_handle.emit("chat-event", &event);
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

    /// 向前端发送 `ChatEvent`。
    fn emit_event(&self, event: &ChatEvent) -> Result<(), ChannelError> {
        self.app_handle
            .emit("chat-event", event)
            .map_err(|e| ChannelError::SendFailed {
                name: "tauri".into(),
                reason: e.to_string(),
            })
    }

    /// 将 `StatusUpdate` 映射为前端 `ChatEvent`。
    pub(crate) fn status_to_event(status: &StatusUpdate) -> ChatEvent {
        match status {
            StatusUpdate::Thinking(msg) => ChatEvent::Thinking {
                message: msg.clone(),
            },
            StatusUpdate::ToolStarted { name } => ChatEvent::ToolStarted { name: name.clone() },
            StatusUpdate::ToolCompleted {
                name,
                success,
                error,
                ..
            } => ChatEvent::ToolCompleted {
                name: name.clone(),
                success: *success,
                error: error.clone(),
            },
            StatusUpdate::ToolResult { name, preview } => ChatEvent::Status {
                message: format!("[{}] {}", name, preview),
                level: "debug".into(),
            },
            StatusUpdate::StreamChunk(content) => ChatEvent::StreamChunk {
                content: content.clone(),
            },
            StatusUpdate::Status(msg) => ChatEvent::Status {
                message: msg.clone(),
                level: "info".into(),
            },
            StatusUpdate::JobStarted {
                job_id,
                title,
                ..
            } => ChatEvent::JobStatus {
                job_id: job_id.clone(),
                title: title.clone(),
                status: "in_progress".into(),
            },
            StatusUpdate::ApprovalNeeded {
                request_id,
                tool_name,
                description,
                ..
            } => ChatEvent::ApprovalNeeded {
                request_id: request_id.clone(),
                tool_name: tool_name.clone(),
                description: description.clone(),
            },
            StatusUpdate::AuthRequired {
                extension_name,
                instructions,
                ..
            } => ChatEvent::Status {
                message: format!(
                    "Extension '{}' requires authentication{}",
                    extension_name,
                    instructions
                        .as_ref()
                        .map(|i| format!(": {}", i))
                        .unwrap_or_default()
                ),
                level: "warn".into(),
            },
            StatusUpdate::AuthCompleted {
                extension_name,
                success,
                message,
            } => ChatEvent::Status {
                message: format!(
                    "Extension '{}' auth {}: {}",
                    extension_name,
                    if *success { "succeeded" } else { "failed" },
                    message
                ),
                level: if *success { "info" } else { "error" }.into(),
            },
            StatusUpdate::ImageGenerated { data_url, path } => ChatEvent::ImageGenerated {
                data_url: data_url.clone(),
                path: path.clone(),
            },
            StatusUpdate::Suggestions { suggestions } => ChatEvent::Suggestions {
                suggestions: suggestions.clone(),
            },
            // ReasoningUpdate 和 TurnCost 不推送到前端，返回空 debug 事件
            StatusUpdate::ReasoningUpdate { .. } | StatusUpdate::TurnCost { .. } => {
                ChatEvent::Status {
                    message: String::new(),
                    level: "debug".into(),
                }
            }
        }
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
        let rx = self
            .incoming_rx
            .lock()
            .await
            .take()
            .ok_or_else(|| ChannelError::StartupFailed {
                name: "tauri".into(),
                reason: "Channel already started (start() called twice)".into(),
            })?;

        // 通知前端引擎就绪
        let _ = self.emit_event(&ChatEvent::ConnectionStatus {
            connected: true,
            message: "IronClaw engine ready".into(),
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

        let event = ChatEvent::Response {
            message_id: msg.id.to_string(),
            content: response.content,
            thread_id,
            source: "chat".to_string(),
        };
        self.emit_event(&event)
    }

    async fn send_status(
        &self,
        status: StatusUpdate,
        metadata: &serde_json::Value,
    ) -> Result<(), ChannelError> {
        // TurnCost 不推送到前端，只更新对话追踪器（汇总整轮 Token 消耗）
        if let StatusUpdate::TurnCost { input_tokens, output_tokens, .. } = status {
            if let Some(tracker) = &self.conversation_tracker {
                let thread_id = metadata
                    .get("notify_thread_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                tracker.update_last_assistant_tokens(
                    thread_id,
                    "",
                    input_tokens.min(i32::MAX as u64) as i32,
                    output_tokens.min(i32::MAX as u64) as i32,
                );
            }
            return Ok(());
        }

        let event = Self::status_to_event(&status);
        self.emit_event(&event)
    }

    async fn broadcast(
        &self,
        _user_id: &str,
        response: OutgoingResponse,
    ) -> Result<(), ChannelError> {
        let event = ChatEvent::Response {
            message_id: uuid::Uuid::new_v4().to_string(),
            content: response.content,
            thread_id: response.thread_id.unwrap_or_default(),
            source: "routine".to_string(),
        };
        self.emit_event(&event)
    }

    async fn health_check(&self) -> Result<(), ChannelError> {
        // 进程内通信，始终健康。
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), ChannelError> {
        let _ = self.emit_event(&ChatEvent::ConnectionStatus {
            connected: false,
            message: "IronClaw engine shutting down".into(),
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

    /// 验证所有 StatusUpdate 变体都能正确映射为 ChatEvent，不会 panic。
    #[test]
    fn test_status_to_event_covers_all_variants() {
        let cases: Vec<StatusUpdate> = vec![
            StatusUpdate::Thinking("processing...".into()),
            StatusUpdate::ToolStarted {
                name: "shell".into(),
            },
            StatusUpdate::ToolCompleted {
                name: "shell".into(),
                success: true,
                error: None,
                parameters: None,
            },
            StatusUpdate::ToolResult {
                name: "shell".into(),
                preview: "output...".into(),
            },
            StatusUpdate::StreamChunk("hello ".into()),
            StatusUpdate::Status("ready".into()),
            StatusUpdate::JobStarted {
                job_id: "j-1".into(),
                title: "Build".into(),
                browse_url: "http://localhost".into(),
            },
            StatusUpdate::ApprovalNeeded {
                request_id: "r-1".into(),
                tool_name: "rm".into(),
                description: "delete file".into(),
                parameters: serde_json::json!({}),
                allow_always: true,
            },
            StatusUpdate::AuthRequired {
                extension_name: "github".into(),
                instructions: Some("click link".into()),
                auth_url: None,
                setup_url: None,
            },
            StatusUpdate::AuthCompleted {
                extension_name: "github".into(),
                success: true,
                message: "ok".into(),
            },
            StatusUpdate::ImageGenerated {
                data_url: "data:image/png;base64,abc".into(),
                path: Some("/tmp/img.png".into()),
            },
            StatusUpdate::Suggestions {
                suggestions: vec!["try this".into()],
            },
            StatusUpdate::ReasoningUpdate {
                narrative: "Choosing search tool".into(),
                decisions: vec![],
            },
            StatusUpdate::TurnCost {
                input_tokens: 100,
                output_tokens: 50,
                cost_usd: "$0.0010".into(),
            },
        ];

        for status in &cases {
            let event = TauriChannel::status_to_event(status);
            // 验证序列化不会失败
            let json = serde_json::to_string(&event).expect("ChatEvent should serialize");
            assert!(!json.is_empty());
        }
    }

    /// 验证 ChatEvent 序列化格式与前端 TypeScript 类型兼容。
    #[test]
    fn test_chat_event_serialization_format() {
        let event = ChatEvent::Response {
            message_id: "msg-1".into(),
            content: "Hello".into(),
            thread_id: "t-1".into(),
            source: "chat".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "response");
        assert_eq!(json["message_id"], "msg-1");
        assert_eq!(json["content"], "Hello");
        assert_eq!(json["thread_id"], "t-1");
        assert_eq!(json["source"], "chat");
    }

    /// 验证 thinking 事件格式。
    #[test]
    fn test_thinking_event_format() {
        let event = ChatEvent::Thinking {
            message: "analyzing...".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "thinking");
        assert_eq!(json["message"], "analyzing...");
    }

    /// 验证 error 事件格式。
    #[test]
    fn test_error_event_format() {
        let event = ChatEvent::Error {
            message: "timeout".into(),
            code: Some("TIMEOUT".into()),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "error");
        assert_eq!(json["message"], "timeout");
        assert_eq!(json["code"], "TIMEOUT");
    }

    /// 验证 connection_status 事件格式。
    #[test]
    fn test_connection_status_event_format() {
        let event = ChatEvent::ConnectionStatus {
            connected: true,
            message: "ready".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "connection_status");
        assert_eq!(json["connected"], true);
    }

    /// 安全测试：验证 ToolCompleted 失败时 error 字段不包含敏感信息模式。
    #[test]
    fn test_security_tool_completed_error_no_sensitive_leak() {
        let status = StatusUpdate::ToolCompleted {
            name: "secret_save".into(),
            success: false,
            error: Some("db connection failed".into()),
            parameters: Some(r#"{"value": "[REDACTED]"}"#.into()),
        };
        let event = TauriChannel::status_to_event(&status);
        let json = serde_json::to_string(&event).unwrap();
        // parameters 不应出现在前端事件中（TauriChannel 不转发 parameters）
        assert!(!json.contains("REDACTED"));
    }

    /// 契约测试：JobStarted → job_status 事件，字段与前端 TypeScript 类型匹配。
    ///
    /// 前端类型：
    /// ```typescript
    /// { type: 'job_status'; job_id: string; title: string; status: string }
    /// ```
    #[test]
    fn test_contract_job_started_maps_to_job_status_event() {
        let status = StatusUpdate::JobStarted {
            job_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            title: "分析代码库".into(),
            browse_url: "http://localhost".into(),
        };
        let event = TauriChannel::status_to_event(&status);
        let json = serde_json::to_value(&event).expect("should serialize");

        assert_eq!(json["type"], "job_status");
        assert_eq!(json["job_id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["title"], "分析代码库");
        assert_eq!(json["status"], "in_progress");

        let obj = json.as_object().expect("should be object");
        assert_eq!(obj.len(), 4, "job_status should have exactly 4 fields (type + 3)");
    }

    /// 安全审计：job_status 事件不应泄露 user_id 或内部字段。
    #[test]
    fn test_audit_job_status_no_sensitive_fields() {
        let status = StatusUpdate::JobStarted {
            job_id: "job-001".into(),
            title: "任务标题".into(),
            browse_url: "http://localhost".into(),
        };
        let event = TauriChannel::status_to_event(&status);
        let json_str = serde_json::to_string(&event).expect("should serialize");

        assert!(!json_str.contains("user_id"), "user_id should not be exposed");
        assert!(!json_str.contains("browse_url"), "browse_url should not be forwarded to frontend");
    }
}
