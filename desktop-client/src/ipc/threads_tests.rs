//! 线程管理 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（ThreadSummary / ThreadMessage 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（无效 UUID、权限拒绝等）

#[cfg(test)]
mod tests {
    use crate::ipc::threads::{ThreadMessage, ThreadSummary};

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_thread_summary_serialization() {
        let summary = ThreadSummary {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            title: Some("Test Thread".into()),
            message_count: 42,
            started_at: "2025-01-01T00:00:00+00:00".into(),
            last_activity: "2025-01-02T12:00:00+00:00".into(),
            channel: "tauri".into(),
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["title"], "Test Thread");
        assert_eq!(json["message_count"], 42);
        assert_eq!(json["channel"], "tauri");
    }

    #[test]
    fn test_thread_summary_deserialization() {
        let json = r#"{
            "id": "abc-123",
            "title": null,
            "message_count": 0,
            "started_at": "2025-01-01T00:00:00+00:00",
            "last_activity": "2025-01-01T00:00:00+00:00",
            "channel": "web"
        }"#;
        let summary: ThreadSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.id, "abc-123");
        assert!(summary.title.is_none());
        assert_eq!(summary.message_count, 0);
    }

    #[test]
    fn test_thread_message_serialization() {
        let msg = ThreadMessage {
            id: "msg-001".into(),
            role: "user".into(),
            content: "Hello, world!".into(),
            created_at: "2025-06-01T10:30:00+00:00".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello, world!");
    }

    #[test]
    fn test_thread_message_deserialization() {
        let json = r#"{
            "id": "msg-002",
            "role": "assistant",
            "content": "I can help with that.",
            "created_at": "2025-06-01T10:31:00+00:00"
        }"#;
        let msg: ThreadMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "I can help with that.");
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 ThreadSummary 类型定义：
    /// ```typescript
    /// interface ThreadSummary {
    ///   id: string;
    ///   title: string | null;
    ///   message_count: number;
    ///   started_at: string;
    ///   last_activity: string;
    ///   channel: string;
    /// }
    /// ```
    #[test]
    fn test_contract_thread_summary_matches_frontend() {
        let summary = ThreadSummary {
            id: "t-1".into(),
            title: Some("Chat".into()),
            message_count: 5,
            started_at: "2025-01-01T00:00:00+00:00".into(),
            last_activity: "2025-01-02T00:00:00+00:00".into(),
            channel: "tauri".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&summary).unwrap();

        // 验证字段名完全匹配
        assert!(json.get("id").is_some(), "missing 'id'");
        assert!(json.get("title").is_some(), "missing 'title'");
        assert!(
            json.get("message_count").is_some(),
            "missing 'message_count'"
        );
        assert!(json.get("started_at").is_some(), "missing 'started_at'");
        assert!(
            json.get("last_activity").is_some(),
            "missing 'last_activity'"
        );
        assert!(json.get("channel").is_some(), "missing 'channel'");

        // 验证类型
        assert!(json["id"].is_string());
        assert!(json["title"].is_string()); // Some → string
        assert!(json["message_count"].is_number());
        assert!(json["started_at"].is_string());
        assert!(json["last_activity"].is_string());
        assert!(json["channel"].is_string());

        // 验证字段数量
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 6, "ThreadSummary should have exactly 6 fields");
    }

    /// 验证 title 为 None 时序列化为 null（前端 `string | null`）。
    #[test]
    fn test_contract_thread_summary_null_title() {
        let summary = ThreadSummary {
            id: "t-2".into(),
            title: None,
            message_count: 0,
            started_at: "2025-01-01T00:00:00+00:00".into(),
            last_activity: "2025-01-01T00:00:00+00:00".into(),
            channel: "tauri".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&summary).unwrap();
        assert!(
            json["title"].is_null(),
            "None title should serialize to null"
        );
    }

    /// 前端 ThreadMessage 类型定义：
    /// ```typescript
    /// interface ThreadMessage {
    ///   id: string;
    ///   role: string;
    ///   content: string;
    ///   created_at: string;
    /// }
    /// ```
    #[test]
    fn test_contract_thread_message_matches_frontend() {
        let msg = ThreadMessage {
            id: "m-1".into(),
            role: "user".into(),
            content: "test".into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();

        assert!(json["id"].is_string());
        assert!(json["role"].is_string());
        assert!(json["content"].is_string());
        assert!(json["created_at"].is_string());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4, "ThreadMessage should have exactly 4 fields");
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 ThreadSummary 不包含消息内容（防止列表接口泄露对话内容）。
    #[test]
    fn test_audit_thread_summary_no_content_leak() {
        let summary = ThreadSummary {
            id: "t-1".into(),
            title: Some("讨论 API Key 配置".into()),
            message_count: 10,
            started_at: "2025-01-01T00:00:00+00:00".into(),
            last_activity: "2025-01-02T00:00:00+00:00".into(),
            channel: "tauri".into(),
        };
        let json_str = serde_json::to_string(&summary).unwrap();

        // 列表接口不应包含消息内容、owner_id 等敏感字段
        assert!(!json_str.contains("content"));
        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("password"));
        assert!(!json_str.contains("api_key"));
    }

    /// 验证 ThreadMessage 不包含 owner_id 或 thread 元数据。
    #[test]
    fn test_audit_thread_message_no_metadata_leak() {
        let msg = ThreadMessage {
            id: "m-1".into(),
            role: "user".into(),
            content: "Hello".into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
        };
        let json_str = serde_json::to_string(&msg).unwrap();

        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("thread_id"));
        assert!(!json_str.contains("channel"));
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_title() {
        let summary = ThreadSummary {
            id: "t-1".into(),
            title: Some("".into()),
            message_count: 0,
            started_at: "".into(),
            last_activity: "".into(),
            channel: "".into(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ThreadSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, Some("".into()));
    }

    #[test]
    fn test_data_unicode_content_in_message() {
        let msg = ThreadMessage {
            id: "m-1".into(),
            role: "user".into(),
            content: "你好世界 🌍 مرحبا العالم".into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ThreadMessage = serde_json::from_str(&json).unwrap();
        assert!(parsed.content.contains("你好世界"));
        assert!(parsed.content.contains("🌍"));
    }

    #[test]
    fn test_data_large_message_count() {
        let summary = ThreadSummary {
            id: "t-1".into(),
            title: None,
            message_count: i64::MAX,
            started_at: "2025-01-01T00:00:00+00:00".into(),
            last_activity: "2025-01-01T00:00:00+00:00".into(),
            channel: "tauri".into(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ThreadSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.message_count, i64::MAX);
    }

    #[test]
    fn test_data_special_characters_in_content() {
        let msg = ThreadMessage {
            id: "m-1".into(),
            role: "assistant".into(),
            content: r#"Use `SELECT * FROM users WHERE name = "O'Brien"` to query."#.into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ThreadMessage = serde_json::from_str(&json).unwrap();
        assert!(parsed.content.contains("O'Brien"));
    }

    #[test]
    fn test_data_json_injection_in_content() {
        let msg = ThreadMessage {
            id: "m-1".into(),
            role: "user".into(),
            content: r#"{"malicious": true, "inject": "}"}"#.into(),
            created_at: "2025-01-01T00:00:00+00:00".into(),
        };
        // 序列化不应 panic，且反序列化应保持原始内容
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ThreadMessage = serde_json::from_str(&json).unwrap();
        assert!(parsed.content.contains("malicious"));
    }

    // =========================================================================
    // 失败路径测试 — UUID 解析
    // =========================================================================

    #[test]
    fn test_failure_invalid_uuid_format() {
        let result = uuid::Uuid::parse_str("not-a-uuid");
        assert!(result.is_err(), "Invalid UUID should fail to parse");
    }

    #[test]
    fn test_failure_empty_uuid() {
        let result = uuid::Uuid::parse_str("");
        assert!(result.is_err(), "Empty string should fail UUID parse");
    }

    #[test]
    fn test_failure_partial_uuid() {
        let result = uuid::Uuid::parse_str("550e8400-e29b-41d4");
        assert!(result.is_err(), "Partial UUID should fail to parse");
    }

    /// 验证合法 UUID 可以正确解析（正常路径对照）。
    #[test]
    fn test_valid_uuid_parses() {
        let result = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000");
        assert!(result.is_ok());
    }
}
