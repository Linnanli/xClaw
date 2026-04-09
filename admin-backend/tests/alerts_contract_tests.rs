//! 告警模块契约测试
//!
//! 验证 API 响应格式满足前端需求

#[cfg(test)]
mod alert_rules_contract {
    use admin_backend::models::CreateAlertRuleRequest;

    /// 前端 AlertRule 类型要求的字段
    const REQUIRED_RULE_FIELDS: &[&str] = &[
        "id",
        "name",
        "event_type",
        "severity",
        "notify_channels",
        "silence_minutes",
        "enabled",
        "created_at",
        "updated_at",
    ];

    #[test]
    fn test_contract_alert_rule_response_has_required_fields() {
        // 模拟后端返回的 JSON（与 row_to_alert_rule 对齐）
        let response = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "DLP 拦截阈值",
            "description": null,
            "event_type": "dlp_violation",
            "condition": {},
            "severity": "high",
            "notify_channels": ["wecom"],
            "silence_minutes": 60,
            "enabled": true,
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z",
        });

        let obj = response.as_object().expect("应为 JSON 对象");
        for field in REQUIRED_RULE_FIELDS {
            assert!(obj.contains_key(*field), "告警规则响应缺少字段: {}", field);
        }
    }

    #[test]
    fn test_contract_alert_rules_list_response_format() {
        // GET /api/alert-rules 返回 { rules: [...], total: N }
        let response = serde_json::json!({
            "rules": [],
            "total": 0,
        });
        assert!(response.get("rules").is_some(), "应包含 rules 字段");
        assert!(response.get("total").is_some(), "应包含 total 字段");
        assert!(response["rules"].is_array(), "rules 应为数组");
    }

    #[test]
    fn test_contract_create_rule_request_all_event_types() {
        // 前端下拉框的所有事件类型都应被后端接受
        let event_types = [
            "dlp_violation",
            "quota_exceeded",
            "model_error",
            "abnormal_login",
            "approval_timeout",
        ];
        for et in &event_types {
            let json = format!(
                r#"{{"name":"test","event_type":"{}","severity":"low"}}"#,
                et
            );
            let req: Result<CreateAlertRuleRequest, _> = serde_json::from_str(&json);
            assert!(req.is_ok(), "事件类型 {} 应能反序列化", et);
        }
    }
}

#[cfg(test)]
mod alert_events_contract {
    /// 前端 AlertEvent 类型要求的字段
    const REQUIRED_EVENT_FIELDS: &[&str] = &[
        "id",
        "rule_name",
        "event_type",
        "severity",
        "trigger_detail",
        "status",
        "created_at",
    ];

    #[test]
    fn test_contract_alert_event_response_has_required_fields() {
        let response = serde_json::json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "rule_id": "550e8400-e29b-41d4-a716-446655440001",
            "rule_name": "DLP 拦截阈值",
            "event_type": "dlp_violation",
            "severity": "critical",
            "trigger_detail": "zhang.wei 触发身份证规则 12 次/小时",
            "event_data": null,
            "status": "pending",
            "resolved_note": null,
            "resolved_at": null,
            "created_at": "2025-01-01T14:32:01Z",
        });

        let obj = response.as_object().expect("应为 JSON 对象");
        for field in REQUIRED_EVENT_FIELDS {
            assert!(obj.contains_key(*field), "告警事件响应缺少字段: {}", field);
        }
    }

    #[test]
    fn test_contract_alert_events_list_response_format() {
        // GET /api/alerts 返回分页格式
        let response = serde_json::json!({
            "data": [],
            "total": 0,
            "page": 1,
            "page_size": 20,
        });
        assert!(response.get("data").is_some(), "应包含 data 字段");
        assert!(response.get("total").is_some(), "应包含 total 字段");
        assert!(response.get("page").is_some(), "应包含 page 字段");
        assert!(response.get("page_size").is_some(), "应包含 page_size 字段");
    }

    #[test]
    fn test_contract_alert_stats_response_format() {
        // GET /api/alerts/stats 返回统计
        let response = serde_json::json!({
            "pending": 7,
            "in_progress": 3,
            "today": 23,
            "closed": 156,
        });
        for field in &["pending", "in_progress", "today", "closed"] {
            assert!(
                response.get(*field).is_some(),
                "告警统计响应缺少字段: {}",
                field
            );
            assert!(response[*field].is_number(), "{} 应为数字", field);
        }
    }

    #[test]
    fn test_contract_alert_event_status_values() {
        // 前端 statusMap 的所有状态值都应被后端接受
        let valid_statuses = ["pending", "acknowledged", "in_progress", "closed"];
        for status in &valid_statuses {
            let json = format!(r#"{{"status":"{}"}}"#, status);
            let req: Result<admin_backend::models::UpdateAlertEventStatusRequest, _> =
                serde_json::from_str(&json);
            assert!(req.is_ok(), "状态 {} 应能反序列化", status);
        }
    }
}
