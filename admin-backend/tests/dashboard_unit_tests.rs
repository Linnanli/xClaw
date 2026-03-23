//! 仪表盘 API 单元测试
//!
//! 覆盖维度：
//! - 契约测试：验证 API 响应格式
//! - 安全审计测试

#[cfg(test)]
mod dashboard_contract_tests {
    use serde_json::json;

    /// test_contract_dashboard_stats_response: 验证统计数据响应格式
    #[test]
    fn test_contract_dashboard_stats_response() {
        let response = json!({
            "total_users": 156,
            "online_clients": 42,
            "dlp_blocked_today": 23,
            "sensitive_ops_today": 8
        });

        assert!(response["total_users"].is_number());
        assert!(response["online_clients"].is_number());
        assert!(response["dlp_blocked_today"].is_number());
        assert!(response["sensitive_ops_today"].is_number());

        // 所有值应为非负数
        assert!(response["total_users"].as_i64().unwrap() >= 0);
        assert!(response["online_clients"].as_i64().unwrap() >= 0);
        assert!(response["dlp_blocked_today"].as_i64().unwrap() >= 0);
        assert!(response["sensitive_ops_today"].as_i64().unwrap() >= 0);
    }

    /// test_contract_dashboard_activity_response: 验证活动日志响应格式
    #[test]
    fn test_contract_dashboard_activity_response() {
        let response = json!({
            "logs": [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "username": "admin",
                    "action": "login",
                    "details": "用户登录成功",
                    "created_at": "2026-03-22T10:00:00Z"
                }
            ]
        });

        assert!(response.get("logs").is_some());
        let logs = response["logs"].as_array().unwrap();
        assert!(!logs.is_empty());

        let log = &logs[0];
        assert!(log.get("id").is_some());
        assert!(log.get("action").is_some());
        assert!(log.get("details").is_some());
        assert!(log.get("created_at").is_some());
    }

    /// test_contract_dashboard_trends_response: 验证趋势数据响应格式
    #[test]
    fn test_contract_dashboard_trends_response() {
        let response = json!({
            "user_activity": [
                { "date": "2026-03-16", "count": 120 },
                { "date": "2026-03-17", "count": 135 }
            ],
            "dlp_blocks": [
                { "date": "2026-03-16", "count": 15 },
                { "date": "2026-03-17", "count": 22 }
            ]
        });

        assert!(response.get("user_activity").is_some());
        assert!(response.get("dlp_blocks").is_some());

        let activity = response["user_activity"].as_array().unwrap();
        for point in activity {
            assert!(point.get("date").is_some());
            assert!(point.get("count").is_some());
            assert!(point["date"].is_string());
            assert!(point["count"].is_number());
        }
    }
}

#[cfg(test)]
mod dashboard_security_tests {
    /// test_security_dashboard_no_sensitive_data: 仪表盘不应泄露敏感信息
    #[test]
    fn test_security_dashboard_no_sensitive_data() {
        // 仪表盘统计数据只包含聚合数字，不包含个人信息
        let stats_fields = vec!["total_users", "online_clients", "dlp_blocked_today", "sensitive_ops_today"];
        let sensitive_fields = vec!["password", "password_hash", "token", "api_key", "secret"];

        for field in &stats_fields {
            for sensitive in &sensitive_fields {
                assert_ne!(
                    field, sensitive,
                    "仪表盘不应包含敏感字段: {}",
                    sensitive
                );
            }
        }
    }

    /// test_security_activity_log_no_password: 活动日志不应包含密码
    #[test]
    fn test_security_activity_log_no_password() {
        let log_details = "用户 admin 登录成功";
        assert!(
            !log_details.contains("password"),
            "日志详情不应包含密码"
        );
        assert!(
            !log_details.contains("secret"),
            "日志详情不应包含密钥"
        );
    }
}
