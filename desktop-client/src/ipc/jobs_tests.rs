//! 任务管理 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（数据类型与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（无效 UUID、空事件列表等）

#[cfg(test)]
mod tests {
    use crate::ipc::jobs::{JobEvent, JobEventsResponse, JobPromptRequest, JobPromptResponse};

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_job_event_serialization() {
        let event = JobEvent {
            id: 42,
            event_type: "status_change".into(),
            data: serde_json::json!({"from": "pending", "to": "in_progress"}),
            created_at: "2025-01-15T10:30:00+00:00".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["id"], 42);
        assert_eq!(json["event_type"], "status_change");
        assert!(json["data"].is_object());
        assert_eq!(json["created_at"], "2025-01-15T10:30:00+00:00");
    }

    #[test]
    fn test_job_event_deserialization() {
        let json = r#"{
            "id": 1,
            "event_type": "output",
            "data": {"content": "Hello world"},
            "created_at": "2025-01-15T10:30:00+00:00"
        }"#;
        let event: JobEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.id, 1);
        assert_eq!(event.event_type, "output");
    }

    #[test]
    fn test_job_event_roundtrip() {
        let original = JobEvent {
            id: 99,
            event_type: "tool_call".into(),
            data: serde_json::json!({"tool": "read_file", "args": {"path": "/tmp/test"}}),
            created_at: "2025-03-22T08:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: JobEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.event_type, original.event_type);
        assert_eq!(parsed.data, original.data);
    }

    #[test]
    fn test_job_events_response_serialization() {
        let resp = JobEventsResponse {
            job_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            events: vec![
                JobEvent {
                    id: 1,
                    event_type: "started".into(),
                    data: serde_json::json!({}),
                    created_at: "2025-01-15T10:00:00+00:00".into(),
                },
                JobEvent {
                    id: 2,
                    event_type: "completed".into(),
                    data: serde_json::json!({"exit_code": 0}),
                    created_at: "2025-01-15T10:05:00+00:00".into(),
                },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["job_id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["events"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_job_prompt_request_serialization() {
        let req = JobPromptRequest {
            content: "继续执行下一步".into(),
            done: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["content"], "继续执行下一步");
        assert_eq!(json["done"], false);
    }

    #[test]
    fn test_job_prompt_request_done_default() {
        let json = r#"{"content": "test"}"#;
        let req: JobPromptRequest = serde_json::from_str(json).unwrap();
        assert!(!req.done, "done should default to false");
    }

    #[test]
    fn test_job_prompt_response_serialization() {
        let resp = JobPromptResponse {
            status: "sent".into(),
            job_id: "test-uuid".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "sent");
        assert_eq!(json["job_id"], "test-uuid");
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 JobEvent 类型定义：
    /// ```typescript
    /// interface JobEvent {
    ///   id: number;
    ///   event_type: string;
    ///   data: any;
    ///   created_at: string;
    /// }
    /// ```
    #[test]
    fn test_contract_job_event_matches_frontend() {
        let event = JobEvent {
            id: 1,
            event_type: "status_change".into(),
            data: serde_json::json!({"key": "value"}),
            created_at: "2025-01-15T10:30:00+00:00".into(),
        };
        let json = serde_json::to_value(&event).unwrap();

        // 验证字段名
        assert!(json.get("id").is_some(), "missing 'id'");
        assert!(json.get("event_type").is_some(), "missing 'event_type'");
        assert!(json.get("data").is_some(), "missing 'data'");
        assert!(json.get("created_at").is_some(), "missing 'created_at'");

        // 验证类型
        assert!(json["id"].is_number());
        assert!(json["event_type"].is_string());
        assert!(json["created_at"].is_string());

        // 验证字段数量
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 4, "JobEvent should have exactly 4 fields");
    }

    /// 前端 JobEventsResponse 类型定义：
    /// ```typescript
    /// interface JobEventsResponse {
    ///   job_id: string;
    ///   events: JobEvent[];
    /// }
    /// ```
    #[test]
    fn test_contract_job_events_response_matches_frontend() {
        let resp = JobEventsResponse {
            job_id: "test-id".into(),
            events: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();

        assert!(json.get("job_id").is_some(), "missing 'job_id'");
        assert!(json.get("events").is_some(), "missing 'events'");
        assert!(json["job_id"].is_string());
        assert!(json["events"].is_array());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "JobEventsResponse should have exactly 2 fields");
    }

    /// 前端 JobPromptResponse 类型定义：
    /// ```typescript
    /// interface JobPromptResponse {
    ///   status: string;
    ///   job_id: string;
    /// }
    /// ```
    #[test]
    fn test_contract_job_prompt_response_matches_frontend() {
        let resp = JobPromptResponse {
            status: "sent".into(),
            job_id: "test-id".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();

        assert!(json["status"].is_string());
        assert!(json["job_id"].is_string());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "JobPromptResponse should have exactly 2 fields");
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 JobEvent 不包含文件系统路径或凭据。
    #[test]
    fn test_audit_job_event_no_sensitive_fields() {
        let event = JobEvent {
            id: 1,
            event_type: "tool_call".into(),
            data: serde_json::json!({"tool": "read_file"}),
            created_at: "2025-01-15T10:30:00+00:00".into(),
        };
        let json_str = serde_json::to_string(&event).unwrap();

        // 结构体本身不应包含敏感字段名
        assert!(!json_str.contains("\"password\""));
        assert!(!json_str.contains("\"secret\""));
        assert!(!json_str.contains("\"token\""));
        assert!(!json_str.contains("\"api_key\""));
    }

    /// 验证 JobEventsResponse 不泄露内部 UUID 格式以外的标识。
    #[test]
    fn test_audit_job_events_response_no_internal_ids() {
        let resp = JobEventsResponse {
            job_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            events: vec![],
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("user_id"));
        assert!(!json_str.contains("session"));
    }

    /// 验证 JobPromptRequest 不包含注入攻击向量。
    #[test]
    fn test_audit_prompt_content_preserved_as_is() {
        // 确保特殊字符不被转义或截断
        let malicious = "'; DROP TABLE jobs; --";
        let req = JobPromptRequest {
            content: malicious.into(),
            done: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JobPromptRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, malicious, "Content should be preserved verbatim");
    }

    // =========================================================================
    // 失败路径测试
    // =========================================================================

    #[test]
    fn test_failure_empty_events_list() {
        let resp = JobEventsResponse {
            job_id: "test".into(),
            events: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["events"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_failure_empty_job_id() {
        let resp = JobEventsResponse {
            job_id: "".into(),
            events: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["job_id"], "");
    }

    #[test]
    fn test_failure_empty_prompt_content() {
        let req = JobPromptRequest {
            content: "".into(),
            done: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["content"], "");
    }

    #[test]
    fn test_failure_null_data_in_event() {
        let event = JobEvent {
            id: 0,
            event_type: "unknown".into(),
            data: serde_json::Value::Null,
            created_at: "".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: JobEvent = serde_json::from_str(&json).unwrap();
        assert!(parsed.data.is_null());
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_large_event_list() {
        let events: Vec<JobEvent> = (0..500)
            .map(|i| JobEvent {
                id: i,
                event_type: format!("event_{}", i),
                data: serde_json::json!({"index": i}),
                created_at: format!("2025-01-15T10:{:02}:00+00:00", i % 60),
            })
            .collect();
        let resp = JobEventsResponse {
            job_id: "bulk-test".into(),
            events,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: JobEventsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.events.len(), 500);
    }

    #[test]
    fn test_data_unicode_in_prompt() {
        let req = JobPromptRequest {
            content: "请继续执行 🚀 下一步操作".into(),
            done: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JobPromptRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "请继续执行 🚀 下一步操作");
        assert!(parsed.done);
    }

    #[test]
    fn test_data_complex_event_data() {
        let event = JobEvent {
            id: 1,
            event_type: "tool_result".into(),
            data: serde_json::json!({
                "tool": "search",
                "results": [
                    {"path": "/a/b.rs", "score": 0.95},
                    {"path": "/c/d.rs", "score": 0.87}
                ],
                "metadata": {
                    "elapsed_ms": 42,
                    "cached": false
                }
            }),
            created_at: "2025-03-22T12:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: JobEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.data["results"].as_array().unwrap().len(),
            2,
            "Nested arrays should be preserved"
        );
    }

    #[test]
    fn test_data_event_type_variants() {
        let types = [
            "status_change",
            "output",
            "tool_call",
            "tool_result",
            "error",
            "thinking",
            "approval_needed",
            "completed",
        ];
        for event_type in types {
            let event = JobEvent {
                id: 0,
                event_type: event_type.into(),
                data: serde_json::json!({}),
                created_at: "2025-01-01T00:00:00+00:00".into(),
            };
            let json = serde_json::to_value(&event).unwrap();
            assert_eq!(json["event_type"], event_type);
        }
    }

    #[test]
    fn test_data_prompt_done_variants() {
        for done in [true, false] {
            let req = JobPromptRequest {
                content: "test".into(),
                done,
            };
            let json = serde_json::to_string(&req).unwrap();
            let parsed: JobPromptRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.done, done);
        }
    }

    #[test]
    fn test_data_negative_event_id() {
        // i64 可以为负数，验证序列化不出错
        let event = JobEvent {
            id: -1,
            event_type: "test".into(),
            data: serde_json::json!(null),
            created_at: "".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: JobEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, -1);
    }

    // =======================================================================================
    // JobInfoResponse 契约测试（ic_list_jobs 新增类型）
    // =======================================================================================

    /// 前端 JobInfo 类型：
    /// ```typescript
    /// interface JobInfo {
    ///   id: string;
    ///   title: string;
    ///   status: string;
    ///   created_at: string;
    ///   started_at?: string;
    ///   completed_at?: string;
    /// }
    /// ```
    #[test]
    fn test_contract_job_info_response_fields() {
        use crate::ipc::jobs::JobInfoResponse;

        let info = JobInfoResponse {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            title: "分析季度报告".into(),
            status: "completed".into(),
            created_at: "2026-04-05T09:00:00Z".into(),
            started_at: Some("2026-04-05T09:00:01Z".into()),
            completed_at: Some("2026-04-05T09:05:00Z".into()),
        };
        let json = serde_json::to_value(&info).expect("should serialize");
        let obj = json.as_object().expect("should be object");

        assert!(json["id"].is_string());
        assert!(json["title"].is_string());
        assert!(json["status"].is_string());
        assert!(json["created_at"].is_string());
        assert_eq!(obj.len(), 6, "JobInfoResponse should have exactly 6 fields");
    }

    #[test]
    fn test_contract_job_info_response_optional_fields_null_when_absent() {
        use crate::ipc::jobs::JobInfoResponse;

        let info = JobInfoResponse {
            id: "job-001".into(),
            title: "待执行任务".into(),
            status: "pending".into(),
            created_at: "2026-04-05T09:00:00Z".into(),
            started_at: None,
            completed_at: None,
        };
        let json = serde_json::to_value(&info).expect("should serialize");
        assert!(json["started_at"].is_null(), "started_at should be null when absent");
        assert!(json["completed_at"].is_null(), "completed_at should be null when absent");
    }

    #[test]
    fn test_audit_job_info_response_no_sensitive_fields() {
        use crate::ipc::jobs::JobInfoResponse;

        let info = JobInfoResponse {
            id: "job-001".into(),
            title: "任务标题".into(),
            status: "running".into(),
            created_at: "2026-04-05T09:00:00Z".into(),
            started_at: None,
            completed_at: None,
        };
        let json_str = serde_json::to_string(&info).expect("should serialize");
        assert!(!json_str.contains("user_id"), "user_id should not be exposed");
        assert!(!json_str.contains("owner_id"), "owner_id should not be exposed");
        assert!(!json_str.contains("api_key"), "api_key should not be exposed");
    }
}
