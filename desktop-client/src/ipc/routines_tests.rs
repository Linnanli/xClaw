//! 日程管理 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（数据类型与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（无效 UUID、空执行列表等）

#[cfg(test)]
mod tests {
    use crate::ipc::routines::{RoutineRun, RoutineRunsResponse};

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_routine_run_serialization() {
        let run = RoutineRun {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            trigger_type: "Time".into(),
            started_at: "2025-03-22T09:00:00+00:00".into(),
            completed_at: Some("2025-03-22T09:05:00+00:00".into()),
            status: "Completed".into(),
            result_summary: Some("Successfully processed 42 items".into()),
            tokens_used: Some(1500),
            job_id: Some("job-uuid-123".into()),
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["id"], "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(json["trigger_type"], "Time");
        assert_eq!(json["status"], "Completed");
        assert_eq!(json["tokens_used"], 1500);
    }

    #[test]
    fn test_routine_run_deserialization() {
        let json = r#"{
            "id": "abc-123",
            "trigger_type": "Manual",
            "started_at": "2025-03-22T09:00:00+00:00",
            "completed_at": null,
            "status": "Running",
            "result_summary": null,
            "tokens_used": null,
            "job_id": null
        }"#;
        let run: RoutineRun = serde_json::from_str(json).unwrap();
        assert_eq!(run.id, "abc-123");
        assert_eq!(run.trigger_type, "Manual");
        assert!(run.completed_at.is_none());
        assert!(run.result_summary.is_none());
        assert!(run.tokens_used.is_none());
        assert!(run.job_id.is_none());
    }

    #[test]
    fn test_routine_run_roundtrip() {
        let original = RoutineRun {
            id: "run-99".into(),
            trigger_type: "Event".into(),
            started_at: "2025-03-22T12:00:00+00:00".into(),
            completed_at: Some("2025-03-22T12:01:30+00:00".into()),
            status: "Failed".into(),
            result_summary: Some("Timeout after 90s".into()),
            tokens_used: Some(0),
            job_id: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: RoutineRun = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.trigger_type, original.trigger_type);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.result_summary, original.result_summary);
    }

    #[test]
    fn test_routine_runs_response_serialization() {
        let resp = RoutineRunsResponse {
            routine_id: "routine-abc".into(),
            runs: vec![
                RoutineRun {
                    id: "run-1".into(),
                    trigger_type: "Time".into(),
                    started_at: "2025-03-22T09:00:00+00:00".into(),
                    completed_at: Some("2025-03-22T09:05:00+00:00".into()),
                    status: "Completed".into(),
                    result_summary: None,
                    tokens_used: Some(500),
                    job_id: Some("job-1".into()),
                },
                RoutineRun {
                    id: "run-2".into(),
                    trigger_type: "Manual".into(),
                    started_at: "2025-03-22T10:00:00+00:00".into(),
                    completed_at: None,
                    status: "Running".into(),
                    result_summary: None,
                    tokens_used: None,
                    job_id: None,
                },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["routine_id"], "routine-abc");
        assert_eq!(json["runs"].as_array().unwrap().len(), 2);
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 RoutineRun 类型定义：
    /// ```typescript
    /// interface RoutineRun {
    ///   id: string;
    ///   trigger_type: string;
    ///   started_at: string;
    ///   completed_at: string | null;
    ///   status: string;
    ///   result_summary: string | null;
    ///   tokens_used: number | null;
    ///   job_id: string | null;
    /// }
    /// ```
    #[test]
    fn test_contract_routine_run_matches_frontend() {
        let run = RoutineRun {
            id: "test".into(),
            trigger_type: "Time".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: Some("2025-01-01T00:01:00+00:00".into()),
            status: "Completed".into(),
            result_summary: Some("OK".into()),
            tokens_used: Some(100),
            job_id: Some("job-1".into()),
        };
        let json = serde_json::to_value(&run).unwrap();

        // 验证字段名
        assert!(json.get("id").is_some(), "missing 'id'");
        assert!(json.get("trigger_type").is_some(), "missing 'trigger_type'");
        assert!(json.get("started_at").is_some(), "missing 'started_at'");
        assert!(json.get("completed_at").is_some(), "missing 'completed_at'");
        assert!(json.get("status").is_some(), "missing 'status'");
        assert!(json.get("result_summary").is_some(), "missing 'result_summary'");
        assert!(json.get("tokens_used").is_some(), "missing 'tokens_used'");
        assert!(json.get("job_id").is_some(), "missing 'job_id'");

        // 验证类型
        assert!(json["id"].is_string());
        assert!(json["trigger_type"].is_string());
        assert!(json["started_at"].is_string());
        assert!(json["completed_at"].is_string()); // Some → string
        assert!(json["status"].is_string());
        assert!(json["result_summary"].is_string()); // Some → string
        assert!(json["tokens_used"].is_number());
        assert!(json["job_id"].is_string()); // Some → string

        // 验证字段数量
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 8, "RoutineRun should have exactly 8 fields");
    }

    /// 验证 nullable 字段为 None 时序列化为 null。
    #[test]
    fn test_contract_routine_run_null_fields() {
        let run = RoutineRun {
            id: "test".into(),
            trigger_type: "Manual".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Running".into(),
            result_summary: None,
            tokens_used: None,
            job_id: None,
        };
        let json = serde_json::to_value(&run).unwrap();
        assert!(json["completed_at"].is_null());
        assert!(json["result_summary"].is_null());
        assert!(json["tokens_used"].is_null());
        assert!(json["job_id"].is_null());
    }

    /// 前端 RoutineRunsResponse 类型定义：
    /// ```typescript
    /// interface RoutineRunsResponse {
    ///   routine_id: string;
    ///   runs: RoutineRun[];
    /// }
    /// ```
    #[test]
    fn test_contract_routine_runs_response_matches_frontend() {
        let resp = RoutineRunsResponse {
            routine_id: "test-id".into(),
            runs: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();

        assert!(json.get("routine_id").is_some(), "missing 'routine_id'");
        assert!(json.get("runs").is_some(), "missing 'runs'");
        assert!(json["routine_id"].is_string());
        assert!(json["runs"].is_array());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "RoutineRunsResponse should have exactly 2 fields");
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 RoutineRun 不包含敏感字段。
    #[test]
    fn test_audit_routine_run_no_sensitive_fields() {
        let run = RoutineRun {
            id: "run-1".into(),
            trigger_type: "Time".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Completed".into(),
            result_summary: Some("Done".into()),
            tokens_used: Some(100),
            job_id: None,
        };
        let json_str = serde_json::to_string(&run).unwrap();

        assert!(!json_str.contains("\"password\""));
        assert!(!json_str.contains("\"secret\""));
        assert!(!json_str.contains("\"token\""));
        assert!(!json_str.contains("\"api_key\""));
        assert!(!json_str.contains("\"owner_id\""));
        assert!(!json_str.contains("\"user_id\""));
    }

    /// 验证 RoutineRunsResponse 不泄露内部标识。
    #[test]
    fn test_audit_routine_runs_response_no_internal_ids() {
        let resp = RoutineRunsResponse {
            routine_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            runs: vec![],
        };
        let json_str = serde_json::to_string(&resp).unwrap();

        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("session"));
        assert!(!json_str.contains("config"));
    }

    /// 验证 result_summary 不包含文件系统路径。
    #[test]
    fn test_audit_result_summary_no_path_leak() {
        let run = RoutineRun {
            id: "run-1".into(),
            trigger_type: "Time".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Completed".into(),
            result_summary: Some("Processed 10 items successfully".into()),
            tokens_used: None,
            job_id: None,
        };
        let json_str = serde_json::to_string(&run).unwrap();

        // result_summary 不应包含绝对路径
        assert!(!json_str.contains("/home/"));
        assert!(!json_str.contains("/Users/"));
        assert!(!json_str.contains("C:\\\\"));
    }

    // =========================================================================
    // 失败路径测试
    // =========================================================================

    #[test]
    fn test_failure_empty_runs_list() {
        let resp = RoutineRunsResponse {
            routine_id: "test".into(),
            runs: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["runs"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_failure_empty_routine_id() {
        let resp = RoutineRunsResponse {
            routine_id: "".into(),
            runs: vec![],
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["routine_id"], "");
    }

    #[test]
    fn test_failure_empty_result_summary() {
        let run = RoutineRun {
            id: "run-1".into(),
            trigger_type: "Manual".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Failed".into(),
            result_summary: Some("".into()),
            tokens_used: None,
            job_id: None,
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["result_summary"], "");
    }

    #[test]
    fn test_failure_zero_tokens_used() {
        let run = RoutineRun {
            id: "run-1".into(),
            trigger_type: "Time".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Completed".into(),
            result_summary: None,
            tokens_used: Some(0),
            job_id: None,
        };
        let json = serde_json::to_value(&run).unwrap();
        assert_eq!(json["tokens_used"], 0);
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_large_runs_list() {
        let runs: Vec<RoutineRun> = (0..200)
            .map(|i| RoutineRun {
                id: format!("run-{}", i),
                trigger_type: "Time".into(),
                started_at: format!("2025-01-{:02}T09:00:00+00:00", (i % 28) + 1),
                completed_at: if i % 2 == 0 {
                    Some(format!("2025-01-{:02}T09:05:00+00:00", (i % 28) + 1))
                } else {
                    None
                },
                status: if i % 3 == 0 { "Completed" } else { "Failed" }.into(),
                result_summary: if i % 4 == 0 {
                    Some(format!("Batch {} done", i))
                } else {
                    None
                },
                tokens_used: if i % 2 == 0 { Some(i * 10) } else { None },
                job_id: if i % 5 == 0 {
                    Some(format!("job-{}", i))
                } else {
                    None
                },
            })
            .collect();
        let resp = RoutineRunsResponse {
            routine_id: "bulk-test".into(),
            runs,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: RoutineRunsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.runs.len(), 200);
    }

    #[test]
    fn test_data_unicode_in_result_summary() {
        let run = RoutineRun {
            id: "run-1".into(),
            trigger_type: "Manual".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Completed".into(),
            result_summary: Some("处理完成 ✅ 共 42 条记录".into()),
            tokens_used: Some(500),
            job_id: None,
        };
        let json = serde_json::to_string(&run).unwrap();
        let parsed: RoutineRun = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.result_summary, Some("处理完成 ✅ 共 42 条记录".into()));
    }

    #[test]
    fn test_data_trigger_type_variants() {
        let types = ["Time", "Manual", "Event", "Webhook", "Cron"];
        for trigger_type in types {
            let run = RoutineRun {
                id: "test".into(),
                trigger_type: trigger_type.into(),
                started_at: "2025-01-01T00:00:00+00:00".into(),
                completed_at: None,
                status: "Completed".into(),
                result_summary: None,
                tokens_used: None,
                job_id: None,
            };
            let json = serde_json::to_value(&run).unwrap();
            assert_eq!(json["trigger_type"], trigger_type);
        }
    }

    #[test]
    fn test_data_status_variants() {
        let statuses = ["Running", "Completed", "Failed", "Cancelled", "Timeout"];
        for status in statuses {
            let run = RoutineRun {
                id: "test".into(),
                trigger_type: "Manual".into(),
                started_at: "2025-01-01T00:00:00+00:00".into(),
                completed_at: None,
                status: status.into(),
                result_summary: None,
                tokens_used: None,
                job_id: None,
            };
            let json = serde_json::to_value(&run).unwrap();
            assert_eq!(json["status"], status);
        }
    }

    #[test]
    fn test_data_large_tokens_used() {
        let run = RoutineRun {
            id: "test".into(),
            trigger_type: "Time".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Completed".into(),
            result_summary: None,
            tokens_used: Some(i32::MAX),
            job_id: None,
        };
        let json = serde_json::to_string(&run).unwrap();
        let parsed: RoutineRun = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tokens_used, Some(i32::MAX));
    }

    #[test]
    fn test_data_negative_tokens_used() {
        // i32 可以为负数，验证序列化不出错
        let run = RoutineRun {
            id: "test".into(),
            trigger_type: "Manual".into(),
            started_at: "2025-01-01T00:00:00+00:00".into(),
            completed_at: None,
            status: "Failed".into(),
            result_summary: None,
            tokens_used: Some(-1),
            job_id: None,
        };
        let json = serde_json::to_string(&run).unwrap();
        let parsed: RoutineRun = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tokens_used, Some(-1));
    }
}
