//! SSE (Server-Sent Events) 客户端
//!
//! 用于连接到后端的 SSE 事件流，接收实时消息和事件。

use std::sync::Arc;
use tokio::sync::RwLock;

/// SSE 事件类型
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SseEvent {
    /// 新消息事件
    #[serde(rename = "message")]
    Message {
        thread_id: String,
        message_id: String,
        content: String,
        role: String,
    },
    /// 消息更新事件
    #[serde(rename = "message_update")]
    MessageUpdate {
        message_id: String,
        content: String,
    },
    /// 对话状态变化事件
    #[serde(rename = "thread_state")]
    ThreadState {
        thread_id: String,
        state: String,
    },
    /// 认证完成事件
    #[serde(rename = "auth_completed")]
    AuthCompleted {
        extension_name: String,
        success: bool,
    },
    /// 认证需求事件
    #[serde(rename = "auth_required")]
    AuthRequired {
        extension_name: String,
        instructions: Option<String>,
    },
    /// 其他事件
    #[serde(rename = "other")]
    Other(String),
}

/// SSE 事件处理器回调
pub type EventHandler = Arc<dyn Fn(SseEvent) + Send + Sync>;

/// SSE 客户端
pub struct SseClient {
    base_url: String,
    auth_token: String,
    event_handlers: Arc<RwLock<Vec<EventHandler>>>,
    is_connected: Arc<RwLock<bool>>,
}

impl SseClient {
    /// 创建新的 SSE 客户端
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self {
            base_url,
            auth_token,
            event_handlers: Arc::new(RwLock::new(Vec::new())),
            is_connected: Arc::new(RwLock::new(false)),
        }
    }

    /// 注册事件处理器
    pub async fn on_event<F>(&self, handler: F)
    where
        F: Fn(SseEvent) + Send + Sync + 'static,
    {
        let mut handlers = self.event_handlers.write().await;
        handlers.push(Arc::new(handler));
    }

    /// 连接到 SSE 事件流
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/api/chat/events", self.base_url);
        
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to connect to SSE: {}", response.status()).into());
        }

        *self.is_connected.write().await = true;

        // 处理 SSE 事件流
        let text = response.text().await?;
        let mut buffer = String::new();

        for line in text.lines() {
            buffer.push_str(line);
            buffer.push('\n');

            // 处理完整的事件行
            if line.starts_with("data: ") {
                let data = &line[6..];
                if let Ok(event) = serde_json::from_str::<SseEvent>(data) {
                    // 调用所有注册的事件处理器
                    let handlers = self.event_handlers.read().await;
                    for handler in handlers.iter() {
                        handler(event.clone());
                    }
                }
            }
        }

        *self.is_connected.write().await = false;
        Ok(())
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// 断开连接
    pub async fn disconnect(&self) {
        *self.is_connected.write().await = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_client_creation() {
        let client = SseClient::new(
            "http://localhost:3000".to_string(),
            "test-token".to_string(),
        );
        assert_eq!(client.base_url, "http://localhost:3000");
        assert_eq!(client.auth_token, "test-token");
    }

    #[tokio::test]
    async fn test_sse_client_not_connected_initially() {
        let client = SseClient::new(
            "http://localhost:3000".to_string(),
            "test-token".to_string(),
        );
        assert!(!client.is_connected().await);
    }

    #[test]
    fn test_sse_event_serialization() {
        let event = SseEvent::Message {
            thread_id: "thread-123".to_string(),
            message_id: "msg-456".to_string(),
            content: "Hello".to_string(),
            role: "user".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("thread-123"));
        assert!(json.contains("msg-456"));
    }

    #[test]
    fn test_sse_event_deserialization() {
        let json = r#"{"message":{"thread_id":"thread-123","message_id":"msg-456","content":"Hello","role":"user"}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        
        match event {
            SseEvent::Message { thread_id, message_id, content, role } => {
                assert_eq!(thread_id, "thread-123");
                assert_eq!(message_id, "msg-456");
                assert_eq!(content, "Hello");
                assert_eq!(role, "user");
            }
            _ => panic!("Expected Message event"),
        }
    }
}
