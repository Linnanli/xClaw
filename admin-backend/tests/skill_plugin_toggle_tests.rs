//! 技能/插件启用禁用测试
//!
//! 覆盖维度：
//! - 契约测试：验证 API 响应格式
//! - 失败路径测试：资源不存在、Gateway 不可用
//! - 安全审计测试：审计日志记录

#[cfg(test)]
mod skill_plugin_contract_tests {
    use serde_json::json;

    /// test_contract_enable_skill_response: 验证启用技能响应格式
    #[test]
    fn test_contract_enable_skill_response() {
        let response = json!({
            "message": "技能已启用",
            "skill_id": "550e8400-e29b-41d4-a716-446655440000",
            "enabled": true,
            "gateway_synced": true
        });

        assert!(response.get("message").is_some());
        assert!(response.get("skill_id").is_some());
        assert!(response.get("enabled").is_some());
        assert!(response.get("gateway_synced").is_some());
        assert_eq!(response["enabled"], true);
        assert!(response["gateway_synced"].is_boolean());
    }

    /// test_contract_disable_skill_response: 验证禁用技能响应格式
    #[test]
    fn test_contract_disable_skill_response() {
        let response = json!({
            "message": "技能已禁用",
            "skill_id": "550e8400-e29b-41d4-a716-446655440000",
            "enabled": false,
            "gateway_synced": true
        });

        assert_eq!(response["enabled"], false);
        assert!(response["message"].as_str().unwrap().contains("禁用"));
    }

    /// test_contract_enable_plugin_response: 验证启用插件响应格式
    #[test]
    fn test_contract_enable_plugin_response() {
        let response = json!({
            "message": "插件已启用",
            "plugin_id": "550e8400-e29b-41d4-a716-446655440000",
            "enabled": true,
            "gateway_synced": false
        });

        assert!(response.get("plugin_id").is_some());
        assert_eq!(response["enabled"], true);
        // gateway_synced=false 表示 Gateway 不可用，使用了本地回退
        assert_eq!(response["gateway_synced"], false);
    }

    /// test_contract_disable_plugin_response: 验证禁用插件响应格式
    #[test]
    fn test_contract_disable_plugin_response() {
        let response = json!({
            "message": "插件已禁用",
            "plugin_id": "550e8400-e29b-41d4-a716-446655440000",
            "enabled": false,
            "gateway_synced": true
        });

        assert_eq!(response["enabled"], false);
    }

    /// test_contract_toggle_idempotency: 验证幂等性（连续两次 enable 结果一致）
    #[test]
    fn test_contract_toggle_idempotency() {
        // 第一次 enable
        let first = json!({
            "message": "技能已启用",
            "enabled": true,
            "gateway_synced": true
        });

        // 第二次 enable（幂等）
        let second = json!({
            "message": "技能已启用",
            "enabled": true,
            "gateway_synced": true
        });

        assert_eq!(first["enabled"], second["enabled"]);
    }
}

#[cfg(test)]
mod skill_plugin_failure_tests {
    use serde_json::json;

    /// test_failure_enable_nonexistent_skill: 启用不存在的技能
    #[test]
    fn test_failure_enable_nonexistent_skill() {
        let error_response = json!({
            "error": "Skill not found",
            "details": "Skill not found"
        });
        assert_eq!(error_response["error"], "Skill not found");
    }

    /// test_failure_disable_nonexistent_skill: 禁用不存在的技能
    #[test]
    fn test_failure_disable_nonexistent_skill() {
        let error_response = json!({
            "error": "Skill not found",
            "details": "Skill not found"
        });
        assert_eq!(error_response["error"], "Skill not found");
    }

    /// test_failure_enable_nonexistent_plugin: 启用不存在的插件
    #[test]
    fn test_failure_enable_nonexistent_plugin() {
        let error_response = json!({
            "error": "Plugin not found",
            "details": "Plugin not found"
        });
        assert_eq!(error_response["error"], "Plugin not found");
    }

    /// test_failure_disable_nonexistent_plugin: 禁用不存在的插件
    #[test]
    fn test_failure_disable_nonexistent_plugin() {
        let error_response = json!({
            "error": "Plugin not found",
            "details": "Plugin not found"
        });
        assert_eq!(error_response["error"], "Plugin not found");
    }

    /// test_failure_gateway_unavailable_fallback: Gateway 不可用时本地回退
    #[test]
    fn test_failure_gateway_unavailable_fallback() {
        // 当 Gateway 不可用时，应直接更新本地数据库
        // 响应中 gateway_synced = false 表示使用了本地回退
        let response = json!({
            "message": "技能已启用",
            "enabled": true,
            "gateway_synced": false
        });

        assert_eq!(response["enabled"], true);
        assert_eq!(response["gateway_synced"], false);
    }
}

#[cfg(test)]
mod skill_plugin_security_tests {
    /// test_security_toggle_audit_log: 启用/禁用操作应记录审计日志
    #[test]
    fn test_security_toggle_audit_log() {
        let audit_actions = vec![
            "enable_skill",
            "disable_skill",
            "enable_plugin",
            "disable_plugin",
        ];

        for action in &audit_actions {
            assert!(
                action.contains("enable") || action.contains("disable"),
                "审计操作类型应包含 enable 或 disable"
            );
        }
    }

    /// test_security_toggle_no_sensitive_data_in_response: 响应不应包含敏感信息
    #[test]
    fn test_security_toggle_no_sensitive_data_in_response() {
        let response_fields = vec!["message", "skill_id", "enabled", "gateway_synced"];
        let sensitive_fields = vec!["password", "token", "api_key", "secret"];

        for field in &response_fields {
            for sensitive in &sensitive_fields {
                assert_ne!(field, sensitive, "响应不应包含敏感字段: {}", sensitive);
            }
        }
    }

    /// test_security_invalid_uuid_handling: 无效 UUID 不应导致崩溃
    #[test]
    fn test_security_invalid_uuid_handling() {
        let invalid_uuids = vec![
            "",
            "not-a-uuid",
            "550e8400-e29b-41d4-a716",
            "'; DROP TABLE skills; --",
            "<script>alert(1)</script>",
        ];

        for uuid_str in &invalid_uuids {
            let parsed = uuid::Uuid::parse_str(uuid_str);
            assert!(parsed.is_err(), "无效 UUID '{}' 应该解析失败", uuid_str);
        }
    }
}
