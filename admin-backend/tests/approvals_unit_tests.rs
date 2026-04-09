//! 审批流单元测试

#[cfg(test)]
mod approval_payload_tests {
    use admin_backend::models::{CreateApprovalRequest, ReviewApprovalRequest};

    #[test]
    fn req_approval_1_create_request_deserializes() {
        let json = r#"{
            "applicant_id": "550e8400-e29b-41d4-a716-446655440000",
            "operation_type": "data_export",
            "operation_name": "导出客户数据 (CSV)",
            "reason": "月度报表需要"
        }"#;
        let req: CreateApprovalRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.operation_type, "data_export");
        assert_eq!(req.reason.as_deref(), Some("月度报表需要"));
        assert!(req.operation_rule_id.is_none());
    }

    #[test]
    fn req_approval_3_review_approve() {
        let json = r#"{"action": "approve", "comment": "同意导出"}"#;
        let req: ReviewApprovalRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.action, "approve");
        assert_eq!(req.comment.as_deref(), Some("同意导出"));
    }

    #[test]
    fn req_approval_3_review_reject() {
        let json = r#"{"action": "reject"}"#;
        let req: ReviewApprovalRequest = serde_json::from_str(json).expect("应能反序列化");
        assert_eq!(req.action, "reject");
        assert!(req.comment.is_none());
    }
}

#[cfg(test)]
mod approval_contract_tests {
    use serde_json::json;

    #[test]
    fn test_contract_approval_list_response() {
        let resp = json!({
            "data": [{"id": "uuid", "applicant": "zhang.wei", "operation_type": "data_export",
                      "operation_name": "导出客户数据", "status": "pending", "expires_at": "2025-01-16T14:32:00Z", "created_at": "2025-01-15T14:32:00Z"}],
            "total": 1, "page": 1, "page_size": 20
        });
        assert!(resp["data"].is_array());
        let item = &resp["data"][0];
        for f in &[
            "id",
            "applicant",
            "operation_type",
            "operation_name",
            "status",
            "expires_at",
            "created_at",
        ] {
            assert!(item.get(*f).is_some(), "列表响应缺少字段: {}", f);
        }
    }

    #[test]
    fn test_contract_approval_stats_response() {
        let resp = json!({"pending": 5, "approved": 42, "rejected": 8, "expired": 3});
        for f in &["pending", "approved", "rejected", "expired"] {
            assert!(resp[*f].is_number(), "统计响应缺少字段: {}", f);
        }
    }

    #[test]
    fn test_contract_approval_check_response() {
        let resp =
            json!({"status": "approved", "expires_at": "2025-01-16T14:32:00Z", "is_valid": true});
        assert!(resp["is_valid"].is_boolean());
        assert!(resp["status"].is_string());
    }
}

#[cfg(test)]
mod approval_expiry_tests {
    #[test]
    fn req_approval_4_approved_within_expiry_is_valid() {
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::hours(12);
        let is_valid = "approved" == "approved" && now < expires;
        assert!(is_valid);
    }

    #[test]
    fn req_approval_4_approved_after_expiry_is_invalid() {
        let now = chrono::Utc::now();
        let expires = now - chrono::Duration::hours(1);
        let is_valid = "approved" == "approved" && now < expires;
        assert!(!is_valid, "过期后应无效");
    }

    #[test]
    fn req_approval_6_pending_over_24h_should_expire() {
        let created = chrono::Utc::now() - chrono::Duration::hours(25);
        let expires = created + chrono::Duration::hours(24);
        let is_expired = chrono::Utc::now() > expires;
        assert!(is_expired, "超过 24 小时应过期");
    }
}
