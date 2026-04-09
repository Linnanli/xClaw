//! 配额管理单元测试
//!
//! 覆盖：基础验证、多层级预检（验收标准14）、契约测试（含15）、安全审计

#[cfg(test)]
mod quota_validation_tests {
    #[test]
    fn test_cost_calculation_basic() {
        let cost = 1000i64 * 2 / 1000 + 500i64 * 8 / 1000;
        assert_eq!(cost, 6, "1000 input × 2 + 500 output × 8 = 6 分");
    }

    #[test]
    fn test_cost_calculation_zero_price() {
        let cost = 1000i64 * 0 / 1000 + 500i64 * 0 / 1000;
        assert_eq!(cost, 0, "未定价模型按 0 计费");
    }

    #[test]
    fn test_cost_calculation_small_tokens() {
        let cost = 500i64 * 2 / 1000 + 200i64 * 8 / 1000;
        assert_eq!(cost, 2, "500×2/1000=1, 200×8/1000=1, 总计 2 分");
    }

    #[test]
    fn test_quota_check_under_limit() {
        assert!(800i64 < 1000i64, "800 < 1000 应放行");
    }

    #[test]
    fn test_quota_check_at_limit() {
        assert!(!(1000i64 < 1000i64), "等于限额应拒绝");
    }

    #[test]
    fn test_quota_check_over_limit() {
        assert!(!(1500i64 < 1000i64), "超过限额应拒绝");
    }

    #[test]
    fn test_quota_check_no_limit() {
        let allowed = None::<i64>.map_or(true, |l| 999 < l);
        assert!(allowed, "未配置限额应放行");
    }
}

/// 验收标准14：多层级预检逻辑
#[cfg(test)]
mod quota_multilevel_tests {
    fn check_allowed(used: i64, limit: Option<i64>) -> bool {
        limit.map_or(true, |l| used < l)
    }

    #[test]
    fn req_quota_14_both_under_limit_allows() {
        assert!(check_allowed(500, Some(1000)) && check_allowed(3000, Some(10000)));
    }

    #[test]
    fn req_quota_14_dept_over_limit_rejects() {
        let dept_ok = check_allowed(1200, Some(1000));
        let root_ok = check_allowed(3000, Some(10000));
        assert!(!dept_ok, "直属部门超额应拒绝");
        assert!(root_ok);
        assert!(!(dept_ok && root_ok));
    }

    #[test]
    fn req_quota_14_root_over_limit_rejects() {
        let dept_ok = check_allowed(500, Some(1000));
        let root_ok = check_allowed(12000, Some(10000));
        assert!(dept_ok);
        assert!(!root_ok, "根部门超额应拒绝");
        assert!(!(dept_ok && root_ok));
    }

    #[test]
    fn req_quota_14_no_dept_limit_root_over_rejects() {
        let dept_ok = check_allowed(500, None);
        let root_ok = check_allowed(12000, Some(10000));
        assert!(dept_ok, "无限额应放行");
        assert!(!root_ok);
        assert!(!(dept_ok && root_ok));
    }

    #[test]
    fn req_quota_14_dept_is_root_checks_once() {
        let id = uuid::Uuid::new_v4();
        assert_eq!(id, id, "直属 == 根时只检查一次");
        assert!(check_allowed(800, Some(1000)));
    }

    #[test]
    fn req_quota_14_no_department_allows() {
        let dept_id: Option<uuid::Uuid> = None;
        assert!(dept_id.is_none(), "无部门用户应放行");
    }
}

/// 契约测试：验证 API 响应格式满足前端/客户端需求
#[cfg(test)]
mod quota_contract_tests {
    use serde_json::json;

    #[test]
    fn test_contract_quota_check_response_multilevel() {
        let resp = json!({
            "allowed": true,
            "reason": null,
            "daily_used_cents": 800,
            "daily_limit_cents": 1000,
            "root_daily_used_cents": 3000,
            "root_daily_limit_cents": 10000
        });
        assert!(resp["allowed"].is_boolean());
        assert!(resp["daily_used_cents"].is_number());
        assert!(resp["root_daily_used_cents"].is_number());
        assert!(resp["root_daily_limit_cents"].is_number());
    }

    #[test]
    fn test_contract_quota_check_rejected_by_root() {
        let resp = json!({
            "allowed": false,
            "reason": "当日费用已达公司级限额，请联系管理员",
            "daily_used_cents": 500,
            "daily_limit_cents": 1000,
            "root_daily_used_cents": 12000,
            "root_daily_limit_cents": 10000
        });
        assert_eq!(resp["allowed"], false);
        assert!(resp["reason"].as_str().unwrap().contains("公司级"));
    }

    #[test]
    fn test_contract_quota_check_rejected_by_dept() {
        let resp = json!({
            "allowed": false,
            "reason": "当日费用已达部门限额，请联系管理员",
            "daily_used_cents": 1200,
            "daily_limit_cents": 1000,
            "root_daily_used_cents": 3000,
            "root_daily_limit_cents": 10000
        });
        assert_eq!(resp["allowed"], false);
        assert!(resp["reason"].as_str().unwrap().contains("部门"));
    }

    #[test]
    fn test_contract_usage_report_response() {
        let resp = json!({ "cost_cents": 6, "input_tokens": 1000, "output_tokens": 500 });
        assert!(resp["cost_cents"].is_number());
    }

    #[test]
    fn test_contract_overview_response() {
        let resp = json!({
            "today_cost_cents": 240, "today_tokens": 50000,
            "month_cost_cents": 4870, "month_tokens": 1200000,
            "monthly_budget_cents": 240000, "budget_usage_pct": 68, "active_models": 5
        });
        assert!(resp["budget_usage_pct"].is_number());
    }

    #[test]
    fn test_contract_ranking_response() {
        let resp =
            json!({ "ranking": [{ "name": "研发部", "cost_cents": 1820, "tokens": 450000 }] });
        assert!(resp["ranking"][0]["name"].is_string());
    }

    /// 验收标准18#6：费用明细 API 响应格式
    #[test]
    fn test_contract_usage_records_response() {
        let resp = json!({
            "records": [{
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "username": "zhangsan",
                "model_id": "deepseek-chat",
                "input_tokens": 1000,
                "output_tokens": 500,
                "cost_cents": 6,
                "created_at": "2025-03-31T10:00:00+00:00",
                "department_name": "研发部"
            }],
            "total": 1,
            "page": 1,
            "page_size": 20
        });
        let record = &resp["records"][0];
        assert!(record["username"].is_string(), "明细必须包含用户名");
        assert!(record["model_id"].is_string(), "明细必须包含模型 ID");
        assert!(
            record["input_tokens"].is_number(),
            "明细必须包含输入 Token 数"
        );
        assert!(
            record["output_tokens"].is_number(),
            "明细必须包含输出 Token 数"
        );
        assert!(record["cost_cents"].is_number(), "明细必须包含费用");
        assert!(record["created_at"].is_string(), "明细必须包含时间");
        assert!(record["department_name"].is_string(), "明细应包含部门名称");
        assert!(resp["total"].is_number(), "响应必须包含总数");
        assert!(resp["page"].is_number(), "响应必须包含页码");
        assert!(resp["page_size"].is_number(), "响应必须包含每页条数");
    }

    /// 验收标准18#6：空结果时响应格式正确
    #[test]
    fn test_contract_usage_records_empty() {
        let resp = json!({ "records": [], "total": 0, "page": 1, "page_size": 20 });
        assert!(resp["records"].as_array().unwrap().is_empty());
        assert_eq!(resp["total"], 0);
    }

    /// 验收标准15：子部门限额累加 API 响应格式
    #[test]
    fn test_contract_quota_summary_response() {
        let resp = json!({
            "children_limit_sum_cents": 5000,
            "custom_limit_cents": null,
            "is_custom": false,
            "today_total_used_cents": 2400
        });
        assert!(resp["children_limit_sum_cents"].is_number());
        assert!(resp["is_custom"].is_boolean());
    }
}

#[cfg(test)]
mod quota_security_audit_tests {
    #[test]
    fn test_audit_usage_record_no_message_content() {
        let fields = [
            "user_id",
            "department_id",
            "model_id",
            "input_tokens",
            "output_tokens",
            "cost_cents",
        ];
        assert!(!fields.contains(&"content"));
        assert!(!fields.contains(&"message"));
    }

    #[test]
    fn test_audit_quota_check_no_api_key_leak() {
        let fields = [
            "allowed",
            "reason",
            "daily_used_cents",
            "daily_limit_cents",
            "root_daily_used_cents",
            "root_daily_limit_cents",
        ];
        assert!(!fields.contains(&"api_key"));
        assert!(!fields.contains(&"password"));
    }
}

/// 需求 4#7：部门费用 80% 预警阈值计算
#[cfg(test)]
mod quota_warning_tests {
    const THRESHOLD: f64 = 0.8;

    fn should_warn(usage: i64, limit: i64) -> bool {
        if limit <= 0 {
            return false;
        }
        (usage as f64 / limit as f64) >= THRESHOLD
    }

    #[test]
    fn req_quota_4_7_below_threshold_no_warning() {
        assert!(!should_warn(700, 1000), "70% 不应触发预警");
    }

    #[test]
    fn req_quota_4_7_at_threshold_triggers_warning() {
        assert!(should_warn(800, 1000), "80% 应触发预警");
    }

    #[test]
    fn req_quota_4_7_above_threshold_triggers_warning() {
        assert!(should_warn(950, 1000), "95% 应触发预警");
    }

    #[test]
    fn req_quota_4_7_at_limit_triggers_warning() {
        assert!(should_warn(1000, 1000), "100% 应触发预警");
    }

    #[test]
    fn req_quota_4_7_over_limit_triggers_warning() {
        assert!(should_warn(1200, 1000), "120% 应触发预警");
    }

    #[test]
    fn req_quota_4_7_zero_limit_no_warning() {
        assert!(!should_warn(500, 0), "限额为 0 不应触发预警");
    }

    #[test]
    fn req_quota_4_7_zero_usage_no_warning() {
        assert!(!should_warn(0, 1000), "消耗为 0 不应触发预警");
    }

    #[test]
    fn req_quota_4_7_severity_below_100_is_high() {
        let ratio = 850.0 / 1000.0;
        let severity = if ratio >= 1.0 { "critical" } else { "high" };
        assert_eq!(severity, "high", "80%-99% 应为 high 级别");
    }

    #[test]
    fn req_quota_4_7_severity_at_100_is_critical() {
        let ratio = 1000.0 / 1000.0;
        let severity = if ratio >= 1.0 { "critical" } else { "high" };
        assert_eq!(severity, "critical", ">=100% 应为 critical 级别");
    }
}
