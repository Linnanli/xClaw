use serde::{Deserialize, Serialize};
use crate::error::Result;

/// API client for communicating with the main server
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
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
pub struct ApprovalRequest {
    pub request_id: String,
    pub action: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTreeResponse {
    pub root: MemoryNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    pub id: String,
    pub name: String,
    pub node_type: String,
    pub children: Vec<MemoryNode>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    pub id: String,
    pub name: String,
    pub content: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetail {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub events: Vec<serde_json::Value>,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_threads(&self) -> Result<ThreadListResponse> {
        let url = format!("{}/api/chat/threads", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<ThreadListResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn create_thread(&self) -> Result<ThreadInfo> {
        let url = format!("{}/api/chat/threads/new", self.base_url);
        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<ThreadInfo>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn send_message(&self, req: SendMessageRequest) -> Result<SendMessageResponse> {
        let url = format!("{}/api/chat/send", self.base_url);
        self.client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<SendMessageResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn approve_operation(&self, req: ApprovalRequest) -> Result<SendMessageResponse> {
        let url = format!("{}/api/chat/approval", self.base_url);
        self.client
            .post(&url)
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
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryTreeResponse>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn read_memory(&self, memory_id: &str) -> Result<MemoryContent> {
        let url = format!("{}/api/memory/read/{}", self.base_url, memory_id);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryContent>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn write_memory(&self, memory_id: &str, content: &str) -> Result<MemoryContent> {
        let url = format!("{}/api/memory/write/{}", self.base_url, memory_id);
        self.client
            .post(&url)
            .json(&serde_json::json!({"content": content}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<MemoryContent>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn search_memory(&self, query: &str) -> Result<Vec<MemoryContent>> {
        let url = format!("{}/api/memory/search", self.base_url);
        self.client
            .get(&url)
            .query(&[("q", query)])
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<MemoryContent>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn get_jobs(&self) -> Result<Vec<JobInfo>> {
        let url = format!("{}/api/jobs", self.base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .json::<Vec<JobInfo>>()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }

    pub async fn get_job_detail(&self, job_id: &str) -> Result<JobDetail> {
        let url = format!("{}/api/jobs/{}", self.base_url, job_id);
        self.client
            .get(&url)
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
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }

    pub async fn restart_job(&self, job_id: &str) -> Result<()> {
        let url = format!("{}/api/jobs/{}/restart", self.base_url, job_id);
        self.client
            .post(&url)
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?;
        Ok(())
    }

    // Log APIs
    pub async fn get_logs(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let url = format!("{}/api/logs", self.base_url);
        self.client
            .get(&url)
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
            .json(&serde_json::json!({"format": format}))
            .send()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))?
            .text()
            .await
            .map_err(|e| crate::error::Error::SerializationError(e.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
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
