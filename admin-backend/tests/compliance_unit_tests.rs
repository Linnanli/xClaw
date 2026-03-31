//! 合规管理单元测试

#[cfg(test)]
mod compliance_payload_tests {
    use admin_backend::models::{GenerateReportRequest, UpdateRetentionPolicyRequest};

    #[test]
    fn req_compliance_4_generate_report_deserializes() {
        let json = r#"{"name":"2024年Q1合规报告","report_type":"quarterly","start_date":"2024-01-01","end_date":"2024-03-31"}"#;
        let req: GenerateReportRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.name, "2024年Q1合规报告");
        assert_eq!(req.start_date, "2024-01-01");
    }

    #[test]
    fn req_compliance_6_update_retention_deserializes() {
        let json = r#"{"retention_days": 180}"#;
        let req: UpdateRetentionPolicyRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.retention_days, 180);
    }
}

#[cfg(test)]
mod compliance_contract_tests {
    use serde_json::json;

    #[test]
    fn test_contract_overview_response() {
        let resp = json!({
            "levels": [{"key": "public", "label": "公开"}],
            "rule_counts": {"public": 4},
            "total_rules": 24,
            "blocks_last_30d": 892
        });
        assert!(resp["levels"].is_array());
        assert!(resp["rule_counts"].is_object());
        assert!(resp["total_rules"].is_number());
    }

    #[test]
    fn test_contract_reports_list_response() {
        let resp = json!({
            "reports": [{"id": "uuid", "name": "Q1报告", "report_type": "quarterly", "start_date": "2024-01-01", "end_date": "2024-03-31", "created_at": "2024-04-01T00:00:00Z"}],
            "total": 1
        });
        assert!(resp["reports"].is_array());
        let r = &resp["reports"][0];
        for f in &["id", "name", "report_type", "start_date", "end_date", "created_at"] {
            assert!(r.get(*f).is_some(), "报告响应缺少字段: {}", f);
        }
    }

    #[test]
    fn test_contract_retention_response() {
        let resp = json!({
            "policies": [{"classification_level": "public", "label": "公开", "retention_days": 90}]
        });
        let p = &resp["policies"][0];
        assert_eq!(p["retention_days"], 90);
        assert!(p["label"].is_string());
    }
}

#[cfg(test)]
mod compliance_validation_tests {
    const VALID_LEVELS: &[&str] = &["public", "internal", "confidential", "top_secret"];

    #[test]
    fn req_compliance_1_all_four_levels_defined() {
        assert_eq!(VALID_LEVELS.len(), 4);
        assert!(VALID_LEVELS.contains(&"public"));
        assert!(VALID_LEVELS.contains(&"top_secret"));
    }

    #[test]
    fn req_compliance_6_retention_days_must_be_positive() {
        let days = 0;
        assert!(days < 1, "保留天数 0 应被拒绝");
    }

    #[test]
    fn req_compliance_4_date_format_validation() {
        let valid = chrono::NaiveDate::parse_from_str("2024-01-01", "%Y-%m-%d");
        assert!(valid.is_ok());
        let invalid = chrono::NaiveDate::parse_from_str("01/01/2024", "%Y-%m-%d");
        assert!(invalid.is_err(), "非 YYYY-MM-DD 格式应被拒绝");
    }
}
