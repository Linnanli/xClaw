use serde::{Deserialize, Serialize};
use crate::error::Result;

/// API client for communicating with the main server
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
    auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub id: String,
    pub state: String,
    pub turn_count: usize,
    pub created_at: String,
    pub updated_at: String,
    pub title: Option<String>,
    pub thread_type: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadListResponse {
    pub assistant_thread: Option<ThreadInfo>,
    pub threads: Vec<ThreadInfo>,
    pub active_thread: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub action: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTreeResponse {
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    pub path: String,
    pub content: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWriteResponse {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub content: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: String,
    #[serde(alias = "state")]
    pub status: String,
    pub created_at: String,
    #[serde(alias = "started_at")]
    pub updated_at: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetail {
    pub id: String,
    #[serde(alias = "state")]
    pub status: String,
    pub created_at: String,
    #[serde(alias = "started_at")]
    pub updated_at: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        // 优先从环境变量读取 token，如果没有则使用默认值
        let auth_token = std::env::var("GATEWAY_AUTH_TOKEN")
            .unwrap_or_else(|_| "59c7c863fa5bd3eeffc94533cd70a3393251c3ada49a226146a5a61ba62d6743".to_string());
        
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token,
        }
    }
    
    pub fn new_with_token(base_url: String, auth_token: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            auth_token,
        }
    }

    /// 获取基础 URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 获取认证令牌
    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.auth_token)
    }

    pub async fn get_threads(&self) -> Result<ThreadListResponse> {
        let url = format!("{}/api/chat/threads", self.base_url);
        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        let status = response.status();
        let text = response.text().await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        if !status.is_success() {
            return Err(crate::error::Error::SerializationError(
                format!("HTTP {}: {}", status, text)
            ));
        }
        
        serde_json::from_str::<ThreadListResponse>(&text)
            .map_err(|e| crate::error::Error::SerializationError(
                format!("Failed to parse response: {} (body: {})", e, text)
            ))
    }

    pub async fn create_thread(&self) -> Result<ThreadInfo> {
        let url = format!("{}/api/chat/thread/new", self.base_url);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<ThreadInfo>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn send_message(&self, req: SendMessageRequest) -> Result<SendMessageResponse> {
        let url = format!("{}/api/chat/send", self.base_url);
        let response = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&req)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        let status = response.status();
        let text = response.text().await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        if !status.is_success() {
            return Err(crate::error::Error::SerializationError(
                format!("HTTP {}: {}", status, text)
            ));
        }
        
        serde_json::from_str::<SendMessageResponse>(&text)
            .map_err(|e| crate::error::Error::SerializationError(
                format!("Failed to parse response: {} (body: {})", e, text)
            ))
    }

    pub async fn get_messages(&self, thread_id: &str) -> Result<Vec<Message>> {
        let url = format!("{}/api/chat/history?thread_id={}", self.base_url, thread_id);
        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        // Parse the HistoryResponse and convert turns to messages
        #[derive(Debug, Deserialize)]
        struct TurnInfo {
            turn_number: i32,
            user_input: String,
            response: Option<String>,
            started_at: String,
            completed_at: Option<String>,
        }
        
        #[derive(Debug, Deserialize)]
        struct HistoryResponse {
            thread_id: String,
            turns: Vec<TurnInfo>,
        }
        
        let history: HistoryResponse = response
            .json()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        let mut messages = Vec::new();
        for turn in history.turns {
            // Add user message
            messages.push(Message {
                id: format!("user-{}", turn.turn_number),
                thread_id: history.thread_id.clone(),
                role: "user".to_string(),
                content: turn.user_input,
                created_at: turn.started_at.clone(),
            });
            
            // Add assistant message if available
            if let Some(response) = turn.response {
                messages.push(Message {
                    id: format!("assistant-{}", turn.turn_number),
                    thread_id: history.thread_id.clone(),
                    role: "assistant".to_string(),
                    content: response,
                    created_at: turn.completed_at.unwrap_or_else(|| turn.started_at.clone()),
                });
            }
        }
        
        Ok(messages)
    }

    pub async fn search_messages(&self, thread_id: &str, query: &str) -> Result<Vec<Message>> {
        let url = format!("{}/api/chat/threads/{}/search?q={}", self.base_url, thread_id, urlencoding::encode(query));
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<Message>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn edit_message(&self, thread_id: &str, message_id: &str, content: &str) -> Result<Message> {
        let url = format!("{}/api/chat/threads/{}/messages/{}", self.base_url, thread_id, message_id);
        self.client
            .put(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"content": content}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Message>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn delete_message(&self, thread_id: &str, message_id: &str) -> Result<()> {
        let url = format!("{}/api/chat/threads/{}/messages/{}", self.base_url, thread_id, message_id);
        self.client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }

    pub async fn export_thread(&self, thread_id: &str, format: &str) -> Result<String> {
        let url = format!("{}/api/chat/threads/{}/export", self.base_url, thread_id);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"format": format}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .text()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn upload_file(&self, thread_id: &str, file_path: &str) -> Result<String> {
        let url = format!("{}/api/chat/threads/{}/files/upload", self.base_url, thread_id);
        let response = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"file_path": file_path}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        response
            .text()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn approve_operation(&self, req: ApprovalRequest) -> Result<SendMessageResponse> {
        let url = format!("{}/api/chat/approval", self.base_url);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&req)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<SendMessageResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn get_memory_tree(&self) -> Result<MemoryTreeResponse> {
        let url = format!("{}/api/memory/tree", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryTreeResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn read_memory(&self, memory_id: &str) -> Result<MemoryContent> {
        let url = format!("{}/api/memory/read", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("path", memory_id)])
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryContent>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn write_memory(&self, memory_id: &str, content: &str) -> Result<MemoryWriteResponse> {
        let url = format!("{}/api/memory/write", self.base_url);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"path": memory_id, "content": content}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryWriteResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn search_memory(&self, query: &str) -> Result<Vec<SearchHit>> {
        let url = format!("{}/api/memory/search", self.base_url);
        
        #[derive(Debug, Serialize, Deserialize)]
        struct SearchResponse {
            results: Vec<SearchHit>,
        }
        
        let response: SearchResponse = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"query": query}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        Ok(response.results)
    }

    pub async fn get_jobs(&self) -> Result<Vec<JobInfo>> {
        let url = format!("{}/api/jobs", self.base_url);
        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<JobListResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        
        Ok(response.jobs)
    }

    pub async fn get_job_detail(&self, job_id: &str) -> Result<JobDetail> {
        let url = format!("{}/api/jobs/{}", self.base_url, job_id);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<JobDetail>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<()> {
        let url = format!("{}/api/jobs/{}/cancel", self.base_url, job_id);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }

    pub async fn restart_job(&self, job_id: &str) -> Result<()> {
        let url = format!("{}/api/jobs/{}/restart", self.base_url, job_id);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }

    pub async fn get_logs(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let url = format!("{}/api/logs", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("limit", limit.to_string().as_str())])
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<LogEntry>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn search_logs(&self, query: &str, limit: usize) -> Result<Vec<LogEntry>> {
        let url = format!("{}/api/logs/search", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<LogEntry>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn filter_logs(&self, level: &str, module: &str, limit: usize) -> Result<Vec<LogEntry>> {
        let url = format!("{}/api/logs/filter", self.base_url);
        self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("level", level), ("module", module), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<LogEntry>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn export_logs(&self, format: &str) -> Result<String> {
        let url = format!("{}/api/logs/export", self.base_url);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&serde_json::json!({"format": format}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .text()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn clear_logs(&self) -> Result<()> {
        let url = format!("{}/api/logs/clear", self.base_url);
        self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_info_deserialization() {
        let json = r#"{
            "id": "thread-123",
            "state": "Idle",
            "turn_count": 5,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T12:00:00Z",
            "title": "Test Thread",
            "thread_type": "assistant",
            "channel": "gateway"
        }"#;

        let thread: ThreadInfo = serde_json::from_str(json).unwrap();
        assert_eq!(thread.id, "thread-123");
        assert_eq!(thread.state, "Idle");
        assert_eq!(thread.turn_count, 5);
        assert_eq!(thread.title, Some("Test Thread".to_string()));
    }

    #[test]
    fn test_thread_list_response_deserialization() {
        let json = r#"{
            "assistant_thread": {
                "id": "assistant-1",
                "state": "Idle",
                "turn_count": 0,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "title": null,
                "thread_type": "assistant",
                "channel": "gateway"
            },
            "threads": [
                {
                    "id": "thread-1",
                    "state": "Idle",
                    "turn_count": 3,
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T10:00:00Z",
                    "title": "First Thread",
                    "thread_type": "thread",
                    "channel": "gateway"
                }
            ],
            "active_thread": "thread-1"
        }"#;

        let response: ThreadListResponse = serde_json::from_str(json).unwrap();
        assert!(response.assistant_thread.is_some());
        assert_eq!(response.threads.len(), 1);
        assert_eq!(response.active_thread, Some("thread-1".to_string()));
    }

    #[test]
    fn test_send_message_request_serialization() {
        let req = SendMessageRequest {
            content: "Hello, world!".to_string(),
            thread_id: Some("thread-123".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Hello, world!"));
        assert!(json.contains("thread-123"));
    }

    #[test]
    fn test_send_message_response_deserialization() {
        let json = r#"{
            "message_id": "msg-456",
            "status": "accepted"
        }"#;

        let response: SendMessageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.message_id, "msg-456");
        assert_eq!(response.status, "accepted");
    }

    #[test]
    fn test_memory_content_deserialization() {
        let json = r#"{
            "id": "mem-123",
            "name": "Project Notes",
            "content": "Important project information",
            "updated_at": "2024-01-01T12:00:00Z"
        }"#;

        let memory: MemoryContent = serde_json::from_str(json).unwrap();
        assert_eq!(memory.id, "mem-123");
        assert_eq!(memory.name, "Project Notes");
        assert_eq!(memory.content, "Important project information");
    }

    #[test]
    fn test_memory_node_deserialization() {
        let json = r#"{
            "id": "root",
            "name": "Root",
            "node_type": "directory",
            "children": [
                {
                    "id": "child-1",
                    "name": "Child 1",
                    "node_type": "file",
                    "children": [],
                    "metadata": null
                }
            ],
            "metadata": null
        }"#;

        let node: MemoryNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.id, "root");
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].name, "Child 1");
    }

    #[test]
    fn test_job_info_deserialization() {
        let json = r#"{
            "id": "job-789",
            "status": "in_progress",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T01:00:00Z",
            "title": "Code Review Task"
        }"#;

        let job: JobInfo = serde_json::from_str(json).unwrap();
        assert_eq!(job.id, "job-789");
        assert_eq!(job.status, "in_progress");
        assert_eq!(job.title, Some("Code Review Task".to_string()));
    }

    #[test]
    fn test_job_detail_deserialization() {
        let json = r#"{
            "id": "job-789",
            "status": "completed",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T02:00:00Z",
            "title": "Code Review Task",
            "description": "Review the pull request",
            "events": []
        }"#;

        let detail: JobDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.id, "job-789");
        assert_eq!(detail.status, "completed");
        assert_eq!(detail.description, Some("Review the pull request".to_string()));
    }

    #[test]
    fn test_approval_request_serialization() {
        let req = ApprovalRequest {
            request_id: "req-123".to_string(),
            action: "approve".to_string(),
            thread_id: Some("thread-456".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("req-123"));
        assert!(json.contains("approve"));
        assert!(json.contains("thread-456"));
    }

    #[test]
    fn test_api_client_creation() {
        let client = ApiClient::new("http://localhost:8000".to_string());
        assert_eq!(client.base_url, "http://localhost:8000");
    }

    #[test]
    fn test_log_entry_deserialization() {
        let json = r#"{
            "timestamp": "2024-01-01T12:00:00Z",
            "level": "INFO",
            "module": "chat",
            "message": "User sent message",
            "context": {"user_id": "123"}
        }"#;

        let log: LogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(log.level, "INFO");
        assert_eq!(log.module, "chat");
        assert_eq!(log.message, "User sent message");
        assert!(log.context.is_some());
    }

    #[test]
    fn test_log_entry_with_null_context() {
        let json = r#"{
            "timestamp": "2024-01-01T12:00:00Z",
            "level": "ERROR",
            "module": "auth",
            "message": "Authentication failed",
            "context": null
        }"#;

        let log: LogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(log.level, "ERROR");
        assert!(log.context.is_none());
    }

    #[test]
    fn test_log_entry_all_levels() {
        let levels = vec!["DEBUG", "INFO", "WARN", "ERROR"];
        
        for level in levels {
            let json = format!(r#"{{
                "timestamp": "2024-01-01T12:00:00Z",
                "level": "{}",
                "module": "test",
                "message": "Test message",
                "context": null
            }}"#, level);

            let log: LogEntry = serde_json::from_str(&json).unwrap();
            assert_eq!(log.level, level);
        }
    }

    #[test]
    fn test_multiple_log_entries_deserialization() {
        let json = r#"[
            {
                "timestamp": "2024-01-01T12:00:00Z",
                "level": "INFO",
                "module": "chat",
                "message": "Message 1",
                "context": null
            },
            {
                "timestamp": "2024-01-01T12:01:00Z",
                "level": "ERROR",
                "module": "auth",
                "message": "Message 2",
                "context": null
            }
        ]"#;

        let logs: Vec<LogEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].level, "INFO");
        assert_eq!(logs[1].level, "ERROR");
    }
}
