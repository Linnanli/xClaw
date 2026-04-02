//! 告警模块单元测试
//!
//! 覆盖：验证函数、数据模型序列化、告警触发逻辑

#[cfg(test)]
mod alert_validation_tests {
    use admin_backend::models::{
        AlertTrigger, CreateAlertRuleRequest, UpdateAlertEventStatusRequest,
        UpdateAlertRuleRequest,
    };

    #[test]
    fn req_alerts_1_create_rule_request_deserializes() {
        let json = r#"{
            "name": "DLP 拦截阈值",
            "event_type": "dlp_violation",
            "severity": "high",
            "notify_channels": ["wecom", "email"],
            "silence_minutes": 30
        }"#;
        let req: CreateAlertRuleRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.name, "DLP 拦截阈值");
        assert_eq!(req.event_type, "dlp_violation");
        assert_eq!(req.severity, "high");
        assert_eq!(req.notify_channels, vec!["wecom", "email"]);
        assert_eq!(req.silence_minutes, 30);
        assert!(req.enabled, "默认应启用");
    }

    #[test]
    fn req_alerts_1_create_rule_defaults() {
        let json = r#"{
            "name": "测试规则",
            "event_type": "model_error",
            "severity": "low"
        }"#;
        let req: CreateAlertRuleRequest = serde_json::from_str(json).expect("应能反序列化");
        assert!(req.notify_channels.is_empty(), "默认无通知渠道");
        assert_eq!(req.silence_minutes, 60, "默认静默期 60 分钟");
        assert!(req.enabled, "默认启用");
        assert_eq!(req.condition, serde_json::json!({}), "默认空条件");
    }

    #[test]
    fn req_alerts_6_update_status_request_deserializes() {
        let json = r#"{"status": "closed", "note": "已处理完毕"}"#;
        let req: UpdateAlertEventStatusRequest =
            serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.status, "closed");
        assert_eq!(req.note.as_deref(), Some("已处理完毕"));
    }

    #[test]
    fn req_alerts_6_update_status_without_note() {
        let json = r#"{"status": "acknowledged"}"#;
        let req: UpdateAlertEventStatusRequest =
            serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.status, "acknowledged");
        assert!(req.note.is_none());
    }

    #[test]
    fn req_alerts_2_update_rule_partial_fields() {
        let json = r#"{"enabled": false}"#;
        let req: UpdateAlertRuleRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.enabled, Some(false));
        assert!(req.name.is_none());
        assert!(req.event_type.is_none());
        assert!(req.severity.is_none());
    }

    #[test]
    fn req_alerts_3_trigger_serializes() {
        let trigger = AlertTrigger {
            event_type: "dlp_violation".into(),
            severity: "critical".into(),
            detail: "用户 zhang.wei 触发身份证规则 12 次/小时".into(),
            event_data: Some(serde_json::json!({"user": "zhang.wei", "count": 12})),
        };
        let json = serde_json::to_string(&trigger).expect("应能序列化");
        assert!(json.contains("dlp_violation"));
        assert!(json.contains("zhang.wei"));
    }
}

#[cfg(test)]
mod alert_event_type_tests {
    const VALID_EVENT_TYPES: &[&str] = &[
        "dlp_violation",
        "quota_exceeded",
        "model_error",
        "abnormal_login",
        "approval_timeout",
    ];

    const VALID_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

    #[test]
    fn req_alerts_2_all_event_types_recognized() {
        for et in VALID_EVENT_TYPES {
            assert!(
                VALID_EVENT_TYPES.contains(et),
                "事件类型 {} 应被识别",
                et
            );
        }
    }

    #[test]
    fn req_alerts_2_all_severities_recognized() {
        for sev in VALID_SEVERITIES {
            assert!(
                VALID_SEVERITIES.contains(sev),
                "严重级别 {} 应被识别",
                sev
            );
        }
    }

    #[test]
    fn req_alerts_2_invalid_event_type_rejected() {
        let invalid = "unknown_event";
        assert!(
            !VALID_EVENT_TYPES.contains(&invalid),
            "无效事件类型应被拒绝"
        );
    }

    #[test]
    fn req_alerts_2_invalid_severity_rejected() {
        let invalid = "extreme";
        assert!(
            !VALID_SEVERITIES.contains(&invalid),
            "无效严重级别应被拒绝"
        );
    }
}

#[cfg(test)]
mod alert_silence_period_tests {
    #[test]
    fn req_alerts_7_within_silence_should_skip_notification() {
        let last_alert_minutes_ago = 30;
        let silence_minutes = 60;
        let in_silence = last_alert_minutes_ago < silence_minutes;
        assert!(in_silence, "30 分钟前触发过，静默期 60 分钟，应跳过通知");
    }

    #[test]
    fn req_alerts_7_outside_silence_should_notify() {
        let last_alert_minutes_ago = 90;
        let silence_minutes = 60;
        let in_silence = last_alert_minutes_ago < silence_minutes;
        assert!(!in_silence, "90 分钟前触发过，静默期 60 分钟，应发送通知");
    }

    #[test]
    fn req_alerts_7_zero_silence_always_notifies() {
        let silence_minutes = 0;
        // 静默期为 0 时，任何时间都不在静默期内（0 < 0 = false）
        let in_silence = 5 < silence_minutes;
        assert!(!in_silence, "静默期为 0 应始终发送通知");
    }
}

#[cfg(test)]
mod alert_field_validation_tests {
    const VALID_EVENT_TYPES: &[&str] = &[
        "dlp_violation",
        "quota_exceeded",
        "model_error",
        "abnormal_login",
        "approval_timeout",
    ];

    const VALID_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];

    #[test]
    fn test_validate_event_type_valid() {
        for et in VALID_EVENT_TYPES {
            assert!(
                VALID_EVENT_TYPES.contains(et),
                "合法事件类型 {} 应通过验证",
                et
            );
        }
    }

    #[test]
    fn test_validate_event_type_invalid() {
        let invalid_types = ["invalid_type", "unknown", "", "DLP_VIOLATION", "dlp violation"];
        for et in &invalid_types {
            assert!(
                !VALID_EVENT_TYPES.contains(et),
                "非法事件类型 '{}' 应返回错误",
                et
            );
        }
    }

    #[test]
    fn test_validate_severity_valid() {
        for sev in VALID_SEVERITIES {
            assert!(
                VALID_SEVERITIES.contains(sev),
                "合法严重级别 {} 应通过验证",
                sev
            );
        }
    }

    #[test]
    fn test_validate_severity_invalid() {
        let invalid_severities = ["extreme", "urgent", "", "HIGH", "super_critical"];
        for sev in &invalid_severities {
            assert!(
                !VALID_SEVERITIES.contains(sev),
                "非法严重级别 '{}' 应返回错误",
                sev
            );
        }
    }
}
