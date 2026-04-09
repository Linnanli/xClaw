//! 定时任务 IPC 命令测试。
//!
//! 覆盖维度（AGENTS.md 测试矩阵）：
//! - 单元测试：RoutineInfo / RoutineRun / CreateRoutineRequest 序列化
//! - 失败路径测试：无效 UUID、无效 trigger JSON、空名称
//! - 契约测试：响应格式与前端 TypeScript 类型匹配
//! - 安全审计测试：不泄露 user_id、内部状态字段

#[cfg(test)]
mod tests {
    use crate::ipc::routines::{
        CreateRoutineRequest, RoutineInfo, RoutineRun, RoutineRunsResponse,
    };

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_routine_info_serialization() {
        let info = RoutineInfo {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            name: "每日报告".into(),
            description: "每天早上生成工作报告".into(),
            status: "active".into(),
            trigger: serde_json::json!({ "type": "cron", "schedule": "0 9 * * *" }),
        };
        let json = serde_json::to_value(&info).expect("should serialize");
        assert_eq!(json["name"], "每日报告");
        assert_eq!(json["status"], "active");
        assert_eq!(json["trigger"]["type"], "cron");
    }

    #[test]
    fn test_routine_info_deserialization() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "test routine",
            "description": "desc",
            "status": "inactive",
            "trigger": {"type": "manual"}
        }"#;
        let info: RoutineInfo = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(info.name, "test routine");
        assert_eq!(info.status, "inactive");
    }

    #[test]
    fn test_routine_run_serialization() {
        let run = RoutineRun {
            id: "run-001".into(),
            trigger_type: "manual".into(),
            started_at: "2026-04-05T10:00:00Z".into(),
            completed_at: Some("2026-04-05T10:00:05Z".into()),
            status: "Ok".into(),
            result_summary: Some("完成".into()),
            tokens_used: Some(150),
            job_id: None,
        };
        let json = serde_json::to_value(&run).expect("should serialize");
        assert_eq!(json["trigger_type"], "manual");
        assert_eq!(json["tokens_used"], 150);
        assert!(json["job_id"].is_null());
    }

    #[test]
    fn test_routine_runs_response_serialization() {
        let resp = RoutineRunsResponse {
            routine_id: "r-001".into(),
            runs: vec![RoutineRun {
                id: "run-1".into(),
                trigger_type: "cron".into(),
                started_at: "2026-04-05T09:00:00Z".into(),
                completed_at: None,
                status: "Running".into(),
                result_summary: None,
                tokens_used: None,
                job_id: None,
            }],
        };
        let json = serde_json::to_value(&resp).expect("should serialize");
        assert_eq!(json["routine_id"], "r-001");
        assert_eq!(json["runs"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_create_routine_request_deserialization_manual() {
        let json = r#"{
            "name": "手动任务",
            "description": "测试",
            "trigger": {"type": "manual"}
        }"#;
        let req: CreateRoutineRequest = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(req.name, "手动任务");
        assert_eq!(req.trigger["type"], "manual");
    }

    #[test]
    fn test_create_routine_request_deserialization_cron() {
        let json = r#"{
            "name": "定时任务",
            "description": "每天执行",
            "trigger": {"type": "cron", "schedule": "0 9 * * *"}
        }"#;
        let req: CreateRoutineRequest = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(req.trigger["schedule"], "0 9 * * *");
    }

    // =========================================================================
    // 失败路径测试
    // =========================================================================

    #[test]
    fn test_failure_invalid_trigger_json_rejected() {
        // trigger 缺少 type 字段，ironclaw Trigger 反序列化应失败
        let trigger_json = serde_json::json!({ "schedule": "0 9 * * *" });
        let result: Result<ironclaw::agent::routine::Trigger, _> =
            serde_json::from_value(trigger_json);
        assert!(result.is_err(), "trigger without 'type' should fail");
    }

    #[test]
    fn test_failure_unknown_trigger_type_rejected() {
        let trigger_json = serde_json::json!({ "type": "unknown_type" });
        let result: Result<ironclaw::agent::routine::Trigger, _> =
            serde_json::from_value(trigger_json);
        assert!(result.is_err(), "unknown trigger type should fail");
    }

    #[test]
    fn test_failure_cron_trigger_missing_schedule() {
        // cron trigger 缺少 schedule 字段
        let trigger_json = serde_json::json!({ "type": "cron" });
        let result: Result<ironclaw::agent::routine::Trigger, _> =
            serde_json::from_value(trigger_json);
        assert!(result.is_err(), "cron trigger without schedule should fail");
    }

    #[test]
    fn test_failure_invalid_uuid_format() {
        // 模拟 ic_routine_runs 中的 UUID 解析失败
        let result = uuid::Uuid::parse_str("not-a-uuid");
        assert!(result.is_err(), "invalid UUID should fail to parse");
    }

    #[test]
    fn test_failure_empty_runs_list() {
        let resp = RoutineRunsResponse {
            routine_id: "r-001".into(),
            runs: vec![],
        };
        let json = serde_json::to_value(&resp).expect("should serialize");
        assert_eq!(json["runs"].as_array().unwrap().len(), 0);
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型匹配
    // =========================================================================

    /// 前端 RoutineInfo 类型：
    /// ```typescript
    /// interface RoutineInfo {
    ///   id: string;
    ///   name: string;
    ///   description: string;
    ///   status: string;
    ///   trigger: any;
    /// }
    /// ```
    #[test]
    fn test_contract_routine_info_fields() {
        let info = RoutineInfo {
            id: "id".into(),
            name: "name".into(),
            description: "desc".into(),
            status: "active".into(),
            trigger: serde_json::json!({"type": "manual"}),
        };
        let json = serde_json::to_value(&info).expect("should serialize");
        let obj = json.as_object().expect("should be object");

        assert!(json["id"].is_string());
        assert!(json["name"].is_string());
        assert!(json["description"].is_string());
        assert!(json["status"].is_string());
        assert!(json["trigger"].is_object());
        assert_eq!(obj.len(), 5, "RoutineInfo should have exactly 5 fields");
    }

    /// 前端 RoutineRun 类型：
    /// ```typescript
    /// interface RoutineRun {
    ///   id: string;
    ///   trigger_type: string;
    ///   started_at: string;
    ///   completed_at?: string;
    ///   status: string;
    ///   result_summary?: string;
    ///   tokens_used?: number;
    ///   job_id?: string;
    /// }
    /// ```
    #[test]
    fn test_contract_routine_run_required_fields() {
        let run = RoutineRun {
            id: "id".into(),
            trigger_type: "manual".into(),
            started_at: "2026-04-05T10:00:00Z".into(),
            completed_at: None,
            status: "Ok".into(),
            result_summary: None,
            tokens_used: None,
            job_id: None,
        };
        let json = serde_json::to_value(&run).expect("should serialize");
        assert!(json["id"].is_string());
        assert!(json["trigger_type"].is_string());
        assert!(json["started_at"].is_string());
        assert!(json["status"].is_string());
    }

    #[test]
    fn test_contract_routine_run_optional_fields_null_when_absent() {
        let run = RoutineRun {
            id: "id".into(),
            trigger_type: "manual".into(),
            started_at: "2026-04-05T10:00:00Z".into(),
            completed_at: None,
            status: "Ok".into(),
            result_summary: None,
            tokens_used: None,
            job_id: None,
        };
        let json = serde_json::to_value(&run).expect("should serialize");
        // 前端期望 null 而非字段缺失
        assert!(json["completed_at"].is_null());
        assert!(json["result_summary"].is_null());
        assert!(json["tokens_used"].is_null());
        assert!(json["job_id"].is_null());
    }

    #[test]
    fn test_contract_status_values() {
        // 验证 status 字段的合法值（active/inactive）
        for status in &["active", "inactive"] {
            let info = RoutineInfo {
                id: "id".into(),
                name: "test".into(),
                description: "".into(),
                status: status.to_string(),
                trigger: serde_json::json!({"type": "manual"}),
            };
            let json = serde_json::to_value(&info).expect("should serialize");
            assert_eq!(json["status"], *status);
        }
    }

    #[test]
    fn test_contract_trigger_types_serialize_correctly() {
        // 验证三种触发器类型都能正确序列化
        let triggers = vec![
            serde_json::json!({"type": "manual"}),
            serde_json::json!({"type": "cron", "schedule": "0 9 * * *"}),
            serde_json::json!({"type": "event", "pattern": ".*"}),
        ];
        for trigger in triggers {
            let info = RoutineInfo {
                id: "id".into(),
                name: "test".into(),
                description: "".into(),
                status: "active".into(),
                trigger: trigger.clone(),
            };
            let json = serde_json::to_value(&info).expect("should serialize");
            assert_eq!(json["trigger"]["type"], trigger["type"]);
        }
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// RoutineInfo 不应泄露 user_id（内部字段）。
    #[test]
    fn test_audit_routine_info_no_user_id_leak() {
        let info = RoutineInfo {
            id: "id".into(),
            name: "test".into(),
            description: "desc".into(),
            status: "active".into(),
            trigger: serde_json::json!({"type": "manual"}),
        };
        let json_str = serde_json::to_string(&info).expect("should serialize");
        assert!(
            !json_str.contains("user_id"),
            "user_id should not be exposed"
        );
        assert!(
            !json_str.contains("owner_id"),
            "owner_id should not be exposed"
        );
    }

    /// RoutineInfo 不应泄露内部运行时状态字段。
    #[test]
    fn test_audit_routine_info_no_internal_state_leak() {
        let info = RoutineInfo {
            id: "id".into(),
            name: "test".into(),
            description: "desc".into(),
            status: "active".into(),
            trigger: serde_json::json!({"type": "manual"}),
        };
        let json_str = serde_json::to_string(&info).expect("should serialize");
        assert!(!json_str.contains("consecutive_failures"));
        assert!(!json_str.contains("run_count"));
        assert!(!json_str.contains("next_fire_at"));
        assert!(!json_str.contains("guardrails"));
        assert!(!json_str.contains("notify"));
    }

    /// RoutineRun 不应泄露敏感的执行详情。
    #[test]
    fn test_audit_routine_run_no_sensitive_data() {
        let run = RoutineRun {
            id: "run-id".into(),
            trigger_type: "cron".into(),
            started_at: "2026-04-05T09:00:00Z".into(),
            completed_at: Some("2026-04-05T09:00:10Z".into()),
            status: "Ok".into(),
            result_summary: Some("任务完成".into()),
            tokens_used: Some(200),
            job_id: None,
        };
        let json_str = serde_json::to_string(&run).expect("should serialize");
        assert!(!json_str.contains("user_id"));
        assert!(!json_str.contains("api_key"));
        assert!(!json_str.contains("secret"));
    }

    // =========================================================================
    // 回归测试 — cron 任务创建时 next_fire_at 必须被计算
    // =========================================================================

    /// 回归测试：cron 触发器创建时 next_fire_at 不能为 None。
    ///
    /// 问题：ic_create_routine 之前硬编码 next_fire_at: None，
    /// 导致引擎的 list_due_cron_routines 查询（WHERE next_fire_at IS NOT NULL）
    /// 永远找不到该任务，cron 任务永远不会被触发。
    #[test]
    fn test_regression_cron_next_fire_at_is_computed_on_create() {
        use ironclaw::agent::routine::next_cron_fire;

        // 模拟 ic_create_routine 中的逻辑：对 cron trigger 计算 next_fire_at
        let schedule = "55 12 * * *"; // 每天 12:55
        let next = next_cron_fire(schedule, None)
            .expect("valid cron expression should not error")
            .expect("cron should always have a next fire time");

        // next_fire_at 必须在未来
        assert!(
            next > chrono::Utc::now(),
            "next_fire_at must be in the future, got: {next}"
        );
    }

    /// manual 和 event 触发器不应计算 next_fire_at（应为 None）。
    #[test]
    fn test_regression_non_cron_triggers_have_no_next_fire_at() {
        use ironclaw::agent::routine::{next_cron_fire, Trigger};

        let non_cron_triggers = vec![
            serde_json::json!({"type": "manual"}),
            serde_json::json!({"type": "event", "pattern": ".*", "channel": null}),
        ];

        for trigger_json in non_cron_triggers {
            let trigger: Trigger =
                serde_json::from_value(trigger_json.clone()).expect("should parse trigger");

            // 只有 Cron 变体才调用 next_cron_fire
            let next_fire_at = match &trigger {
                Trigger::Cron { schedule, timezone } => {
                    next_cron_fire(schedule, timezone.as_deref()).expect("valid cron")
                }
                _ => None,
            };

            assert!(
                next_fire_at.is_none(),
                "non-cron trigger {:?} should have next_fire_at = None",
                trigger_json["type"]
            );
        }
    }

    // ==========================================================================================
    // 契约测试 — TriggerRoutineResponse（ic_fire_routine 返回值，前端依赖字段名）
    // ==========================================================================================

    /// 前端 TriggerRoutineResponse 类型：
    /// ```typescript
    /// interface TriggerRoutineResponse {
    ///   status: string;
    ///   routine_id: string;
    ///   run_id: string;
    /// }
    /// ```
    #[test]
    fn test_contract_fire_routine_response_fields() {
        use crate::ipc::routines::TriggerRoutineResponse;

        let resp = TriggerRoutineResponse {
            status: "triggered".into(),
            routine_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            run_id: "11f5a970-bf31-4d6f-8a20-61f3f96a3c43".into(),
        };
        let json = serde_json::to_value(&resp).expect("should serialize");
        let obj = json.as_object().expect("should be object");

        assert!(json["status"].is_string(), "status must be string");
        assert!(json["routine_id"].is_string(), "routine_id must be string");
        assert!(json["run_id"].is_string(), "run_id must be string");
        assert_eq!(
            obj.len(),
            3,
            "TriggerRoutineResponse should have exactly 3 fields"
        );
    }

    #[test]
    fn test_contract_fire_routine_response_no_extra_fields() {
        use crate::ipc::routines::TriggerRoutineResponse;

        let resp = TriggerRoutineResponse {
            status: "triggered".into(),
            routine_id: "routine-abc".into(),
            run_id: "run-abc".into(),
        };
        let json_str = serde_json::to_string(&resp).expect("should serialize");

        // 不应泄露内部字段
        assert!(!json_str.contains("user_id"));
        assert!(!json_str.contains("owner_id"));
        assert!(!json_str.contains("thread_id"));
        assert!(!json_str.contains("prompt"));
    }
}
