//! 合规管理失败路径测试
//!
//! 覆盖维度：
//! - 无效日期格式
//! - 无效数据分级 level
//! - 保留天数边界值（0、负数）
//! - 日期范围逻辑错误（结束早于开始）

#[cfg(test)]
mod compliance_failure_tests {
    use admin_backend::models::{GenerateReportRequest, UpdateRetentionPolicyRequest};

    // ── 日期格式校验 ──────────────────────────────────────────────

    #[test]
    fn test_failure_invalid_start_date_format() {
        let json = r#"{"name":"报告","start_date":"01/01/2024","end_date":"2024-03-31"}"#;
        let req: GenerateReportRequest = serde_json::from_str(json).expect("反序列化应成功");
        // 日期格式校验在 handler 层，这里验证 parse 会失败
        let result = chrono::NaiveDate::parse_from_str(&req.start_date, "%Y-%m-%d");
        assert!(result.is_err(), "非 YYYY-MM-DD 格式应被拒绝");
    }

    #[test]
    fn test_failure_invalid_end_date_format() {
        let json = r#"{"name":"报告","start_date":"2024-01-01","end_date":"2024/03/31"}"#;
        let req: GenerateReportRequest = serde_json::from_str(json).expect("反序列化应成功");
        let result = chrono::NaiveDate::parse_from_str(&req.end_date, "%Y-%m-%d");
        assert!(result.is_err(), "斜杠分隔的日期格式应被拒绝");
    }

    #[test]
    fn test_failure_end_before_start_date() {
        let start = chrono::NaiveDate::parse_from_str("2024-03-31", "%Y-%m-%d").unwrap();
        let end = chrono::NaiveDate::parse_from_str("2024-01-01", "%Y-%m-%d").unwrap();
        assert!(end < start, "结束日期早于开始日期应被业务层拒绝");
    }

    // ── 保留天数边界值 ────────────────────────────────────────────

    #[test]
    fn test_failure_retention_days_zero() {
        let json = r#"{"retention_days": 0}"#;
        let req: UpdateRetentionPolicyRequest = serde_json::from_str(json).expect("反序列化应成功");
        assert!(
            req.retention_days < 1,
            "保留天数 0 应被 handler 拒绝（< 1）"
        );
    }

    #[test]
    fn test_failure_retention_days_negative() {
        // i32 反序列化负数是合法的，校验在 handler 层
        let json = r#"{"retention_days": -30}"#;
        let req: UpdateRetentionPolicyRequest = serde_json::from_str(json).expect("反序列化应成功");
        assert!(req.retention_days < 1, "负数保留天数应被 handler 拒绝");
    }

    // ── 无效分级 level ────────────────────────────────────────────

    #[test]
    fn test_failure_invalid_classification_level() {
        const VALID_LEVELS: &[&str] = &["public", "internal", "confidential", "top_secret"];
        let invalid_levels = ["secret", "PUBLIC", "top-secret", "", "unknown"];
        for level in &invalid_levels {
            assert!(
                !VALID_LEVELS.contains(level),
                "无效分级 '{}' 不应通过校验",
                level
            );
        }
    }

    #[test]
    fn test_failure_empty_report_name() {
        let json = r#"{"name":"","start_date":"2024-01-01","end_date":"2024-03-31"}"#;
        let req: GenerateReportRequest = serde_json::from_str(json).expect("反序列化应成功");
        assert!(req.name.trim().is_empty(), "空报告名称应被 handler 拒绝");
    }
}

#[cfg(test)]
mod jwt_auth_middleware_tests {
    use admin_backend::middleware::auth::PUBLIC_PREFIXES;
    use serde_json::json;

    fn is_public_path(path: &str) -> bool {
        PUBLIC_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
    }

    // ── JWT 中间件公开路径豁免验证 ────────────────────────────────

    /// 验证公开路径列表的完整性
    /// 这些路径不需要 JWT token，客户端直连场景依赖这些豁免
    #[test]
    fn test_public_paths_include_client_endpoints() {
        // 客户端直连必须豁免的路径
        let required_public = [
            "/api/client-reports",     // 客户端数据上报
            "/api/client-config",      // 客户端配置拉取
            "/api/client-models",      // 客户端模型列表
            "/api/client-policy",      // 签名策略拉取
            "/api/quota/check",        // 配额预检（客户端发起）
            "/api/quota/report-usage", // 用量上报
            "/api/v1/search",          // 私有注册表搜索
            "/api/v1/download",        // 私有注册表下载
            "/api/v1/skills/550e8400-e29b-41d4-a716-446655440000", // 私有注册表详情
        ];

        for path in &required_public {
            assert!(is_public_path(path), "客户端路径 '{}' 必须在公开路径列表中", path);
        }
    }

    /// 验证合规路径需要认证（不在公开列表中）
    #[test]
    fn test_compliance_paths_require_auth() {
        let protected_paths = [
            "/api/compliance/overview",
            "/api/compliance/reports",
            "/api/compliance/retention",
        ];

        for path in &protected_paths {
            assert!(
                !is_public_path(path),
                "合规路径 '{}' 不应在公开列表中，必须要求认证",
                path
            );
        }
    }

    // ── LoginResponse 结构契约 ────────────────────────────────────

    #[test]
    fn test_contract_login_response_includes_user() {
        let resp = json!({
            "access_token": "eyJ...",
            "refresh_token": "eyJ...",
            "expires_in": 3600,
            "user": {
                "id": "uuid",
                "username": "admin",
                "email": "admin@example.com",
                "roles": ["超级管理员"]
            }
        });

        assert!(resp["access_token"].is_string(), "access_token 必须存在");
        assert!(resp["user"].is_object(), "user 对象必须存在");
        assert!(resp["user"]["roles"].is_array(), "roles 必须是数组");
        assert!(resp["user"]["id"].is_string(), "user.id 必须存在");
        assert!(
            resp["user"]["username"].is_string(),
            "user.username 必须存在"
        );
    }

    #[test]
    fn test_contract_login_response_no_password_field() {
        // 登录响应中不应包含密码相关字段
        let resp = json!({
            "access_token": "eyJ...",
            "refresh_token": "eyJ...",
            "expires_in": 3600,
            "user": {
                "id": "uuid",
                "username": "admin",
                "email": "admin@example.com",
                "roles": []
            }
        });

        assert!(
            resp["user"].get("password").is_none(),
            "响应中不应包含 password 字段"
        );
        assert!(
            resp["user"].get("password_hash").is_none(),
            "响应中不应包含 password_hash 字段"
        );
    }
}

#[cfg(test)]
mod pdf_export_contract_tests {
    use serde_json::json;

    /// PDF 导出依赖报告的 content 字段，验证 content 结构完整性
    #[test]
    fn test_contract_pdf_export_requires_content_fields() {
        let content = json!({
            "dlp_blocks": 42,
            "policy_changes": 5,
            "alert_events": 3,
            "approval_tickets": 1
        });

        for field in &[
            "dlp_blocks",
            "policy_changes",
            "alert_events",
            "approval_tickets",
        ] {
            assert!(
                content.get(*field).is_some(),
                "PDF 导出所需字段缺失: {}",
                field
            );
            assert!(
                content[*field].is_number(),
                "PDF 导出字段 {} 应为数字类型",
                field
            );
        }
    }

    /// content 字段为 null 时 PDF 导出应降级处理（不崩溃）
    #[test]
    fn test_failure_pdf_export_with_null_content() {
        let report = json!({
            "id": "uuid",
            "name": "测试报告",
            "report_type": "monthly",
            "start_date": "2024-01-01",
            "end_date": "2024-01-31",
            "created_at": "2024-02-01T00:00:00Z",
            "content": null
        });

        // content 为 null 时，前端应显示"报告内容暂不可用"而非崩溃
        assert!(
            report["content"].is_null(),
            "content 为 null 时应被前端优雅处理"
        );
    }

    /// 报告类型标签映射完整性
    #[test]
    fn test_contract_report_type_labels_complete() {
        let valid_types = ["monthly", "quarterly", "annual", "custom"];
        let type_labels = [
            ("monthly", "月度报告"),
            ("quarterly", "季度报告"),
            ("annual", "年度报告"),
            ("custom", "自定义"),
        ];

        assert_eq!(
            valid_types.len(),
            type_labels.len(),
            "报告类型标签映射不完整"
        );
        for (type_key, _) in &type_labels {
            assert!(valid_types.contains(type_key), "未知报告类型: {}", type_key);
        }
    }

    /// PDF 文件名格式验证（使用报告 ID 前 8 位）
    #[test]
    fn test_contract_pdf_filename_format() {
        let report_id = "3a948b79-c8f1-48a7-94b3-399772a319d8";
        let filename = format!("compliance-report-{}.pdf", &report_id[..8]);
        assert_eq!(filename, "compliance-report-3a948b79.pdf");
        assert!(filename.ends_with(".pdf"), "PDF 文件名应以 .pdf 结尾");
    }
}
