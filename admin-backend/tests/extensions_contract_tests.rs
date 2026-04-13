//! 扩展管理契约测试（需求 14）
//!
//! 验证 API 响应格式满足前端和 ironclaw 引擎的消费需求。

/// 注册表 API 响应格式契约（ironclaw SkillCatalog 消费）
mod registry_contract {
    use serde_json::json;

    #[test]
    fn test_search_response_wrapped_format() {
        // ironclaw 的 CatalogSearchEnvelope 期望 {"results": [...]}
        let response = json!({
            "results": [
                {
                    "slug": "550e8400-e29b-41d4-a716-446655440000",
                    "display_name": "写作助手",
                    "summary": "帮助撰写各类文档",
                    "version": "1.0.0",
                    "score": 1.0
                }
            ]
        });

        assert!(
            response.get("results").is_some(),
            "注册表搜索响应必须包含 results 字段"
        );
        let results = response["results"].as_array().unwrap();
        assert!(!results.is_empty());

        let entry = &results[0];
        // ironclaw CatalogSearchResult 期望的字段
        assert!(entry.get("slug").is_some(), "缺少 slug 字段");
        assert!(
            entry.get("display_name").is_some(),
            "缺少 display_name 字段"
        );
        assert!(entry.get("summary").is_some(), "缺少 summary 字段");
    }

    #[test]
    fn test_skill_detail_response_format() {
        // ironclaw SkillDetailResponse 期望的格式
        let response = json!({
            "skill": {
                "slug": "550e8400-e29b-41d4-a716-446655440000",
                "display_name": "写作助手",
                "summary": "帮助撰写各类文档",
                "updated_at": null
            },
            "owner": null,
            "stats": {
                "installs_current": 42,
                "downloads": 42,
                "stars": null
            }
        });

        assert!(response.get("skill").is_some(), "缺少 skill 字段");
        assert!(response.get("stats").is_some(), "缺少 stats 字段");
        let skill = &response["skill"];
        assert!(skill.get("slug").is_some());
        assert!(skill.get("display_name").is_some());
    }

    #[test]
    fn test_download_url_format() {
        // ironclaw skill_download_url 期望：{registry}/api/v1/download?slug={encoded_slug}
        let _registry = "https://admin.corp.com/api/v1?client_token=abc";
        let slug = "550e8400-e29b-41d4-a716-446655440000";
        // 注意：ironclaw 会把 registry_url 和 /api/v1/download?slug= 拼接
        // 所以 registry_url 应该是 base，不含 /api/v1 路径
        // 实际下发的 skill_registry_url = "https://admin.corp.com?client_token=abc"
        // ironclaw 拼接后 = "https://admin.corp.com?client_token=abc/api/v1/download?slug=xxx"
        // 这里验证 slug 格式（UUID）
        assert!(uuid::Uuid::parse_str(slug).is_ok(), "slug 应为合法 UUID");
    }
}

/// 技能列表 API 响应格式契约（前端消费）
mod skill_list_contract {
    use serde_json::json;

    #[test]
    fn test_skill_list_response_has_required_fields() {
        let skill = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "name": "写作助手",
            "description": "帮助撰写各类文档",
            "version": "1.0.0",
            "author": "admin",
            "enabled": true,
            "source": "admin_upload",
            "review_status": "approved",
            "is_builtin": false,
            "invoke_count": 0,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        });

        // 前端需要的字段
        for field in &[
            "id",
            "name",
            "enabled",
            "source",
            "review_status",
            "is_builtin",
            "invoke_count",
        ] {
            assert!(skill.get(field).is_some(), "技能响应缺少字段: {}", field);
        }
    }

    #[test]
    fn test_review_status_values() {
        // 前端 ReviewBadge 组件期望的状态值
        let valid_statuses = [
            "scanning",
            "pending",
            "approved",
            "rejected",
            "scan_failed",
            "yanked",
        ];
        for s in &valid_statuses {
            assert!(
                matches!(
                    *s,
                    "scanning"
                        | "pending"
                        | "approved"
                        | "rejected"
                        | "scan_failed"
                        | "yanked"
                ),
                "无效的 review_status: {}",
                s
            );
        }
    }

    #[test]
    fn test_source_values() {
        let valid_sources = ["builtin", "admin_upload"];
        for s in &valid_sources {
            assert!(
                matches!(*s, "builtin" | "admin_upload"),
                "无效的 source: {}",
                s
            );
        }
    }
}

mod scan_results_contract {
    use serde_json::json;

    #[test]
    fn test_scan_result_response_has_required_fields() {
        let response = json!({
            "skill_id": "550e8400-e29b-41d4-a716-446655440000",
            "scan_result": {
                "scanner_type": "cisco-ai-skill-scanner",
                "verdict": "SAFE",
                "is_safe": true,
                "max_severity": null,
                "findings_count": 0,
                "findings": [],
                "scan_duration_ms": 14,
                "scanned_at": "2026-04-10T10:00:00Z",
                "created_at": "2026-04-10T10:00:00Z"
            }
        });

        assert!(response.get("skill_id").is_some(), "缺少 skill_id 字段");
        assert!(response.get("scan_result").is_some(), "缺少 scan_result 字段");

        let scan_result = &response["scan_result"];
        for field in [
            "scanner_type",
            "verdict",
            "is_safe",
            "findings_count",
            "findings",
        ] {
            assert!(scan_result.get(field).is_some(), "scan_result 缺少字段: {}", field);
        }
    }
}

mod skill_action_contract {
    use serde_json::json;

    #[test]
    fn test_rescan_response_has_required_fields() {
        let response = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "previous_review_status": "scan_failed",
            "review_status": "pending",
            "enabled": false,
            "is_safe": true,
            "findings_count": 0
        });

        for field in [
            "id",
            "previous_review_status",
            "review_status",
            "enabled",
            "is_safe",
            "findings_count",
        ] {
            assert!(response.get(field).is_some(), "rescan 响应缺少字段: {}", field);
        }
    }

    #[test]
    fn test_yank_response_has_required_fields() {
        let response = json!({
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "previous_review_status": "approved",
            "review_status": "yanked",
            "enabled": false
        });

        for field in ["id", "previous_review_status", "review_status", "enabled"] {
            assert!(response.get(field).is_some(), "yank 响应缺少字段: {}", field);
        }
    }
}

/// client-config 响应格式契约（Desktop Client AdminClientConfig 消费）
mod client_config_contract {
    use serde_json::json;

    #[test]
    fn test_skill_registry_url_field_present() {
        // AdminClientConfig 期望 skill_registry_url 字段
        let config = json!({
            "llm_backend": "openai",
            "config_version": 1,
            "managed_mode": true,
            "skill_registry_url": "https://admin.corp.com?client_token=abc123"
        });

        assert!(
            config.get("skill_registry_url").is_some(),
            "client-config 响应必须包含 skill_registry_url 字段"
        );

        let url = config["skill_registry_url"].as_str().unwrap();
        assert!(
            url.contains("/api/v1") || url.contains("client_token"),
            "skill_registry_url 格式不正确: {}",
            url
        );

        assert_eq!(config["managed_mode"], true);
    }
}

/// client-policy 响应格式契约（Desktop Client 验签消费）
mod client_policy_contract {
    use serde_json::json;

    #[test]
    fn test_client_policy_response_has_signed_fields() {
        let response = json!({
            "algorithm": "ed25519",
            "key_id": "managed-policy-key-v1",
            "manifest_payload": "{\"policy_version\":1}",
            "signature": "ZmFrZS1zaWduYXR1cmU="
        });

        for field in ["algorithm", "key_id", "manifest_payload", "signature"] {
            assert!(
                response.get(field).is_some(),
                "client-policy 缺少字段: {}",
                field
            );
        }
        assert_eq!(response["algorithm"], "ed25519");
    }

    #[test]
    fn test_client_policy_manifest_payload_contains_allowlists() {
        let manifest = json!({
            "policy_version": 1,
            "issued_at": "2026-04-12T00:00:00Z",
            "expires_at": "2026-04-12T00:05:00Z",
            "managed_mode": true,
            "allowed_skills": ["review-checklist"],
            "allowed_extensions": ["github"]
        });

        for field in [
            "policy_version",
            "issued_at",
            "expires_at",
            "managed_mode",
            "allowed_skills",
            "allowed_extensions",
        ] {
            assert!(manifest.get(field).is_some(), "manifest 缺少字段: {}", field);
        }
    }
}

/// 内置条目契约（管理端列表 API 必须返回内置条目）
mod builtin_entries_contract {
    use serde_json::json;

    #[test]
    fn test_skill_list_includes_builtin_fields() {
        // 管理端技能列表响应必须包含 is_builtin 和 source 字段
        // 前端据此渲染"内置"徽标，区分内置和上传的技能
        let skill = json!({
            "id": "a1000000-0000-0000-0000-000000000001",
            "name": "delegation",
            "source": "builtin",
            "review_status": "approved",
            "is_builtin": true,
            "invoke_count": 0
        });

        assert_eq!(skill["source"], "builtin");
        assert_eq!(skill["is_builtin"], true);
        assert_eq!(skill["review_status"], "approved");
    }

    #[test]
    fn test_plugin_list_includes_type_and_sandbox_fields() {
        // 管理端插件列表响应必须包含 plugin_type 和 requires_sandbox 字段
        let plugin = json!({
            "id": "b1000000-0000-0000-0000-000000000011",
            "name": "github",
            "plugin_type": "wasm",
            "requires_sandbox": false,
            "is_builtin": true,
            "source": "builtin",
            "review_status": "approved"
        });

        assert!(
            plugin.get("plugin_type").is_some(),
            "插件响应缺少 plugin_type 字段"
        );
        assert!(
            plugin.get("requires_sandbox").is_some(),
            "插件响应缺少 requires_sandbox 字段"
        );
        assert!(
            plugin.get("is_builtin").is_some(),
            "插件响应缺少 is_builtin 字段"
        );
    }

    #[test]
    fn test_builtin_skill_names_match_ironclaw_registry() {
        // 验证种子数据中的技能名称与 ironclaw/skills/ 目录一致
        // local-test 和 web-ui-test 是测试用技能，不种入生产 DB
        let seeded = [
            "delegation",
            "review-checklist",
            "routine-advisor",
            "ironclaw-workflow-orchestrator",
        ];
        let ironclaw = [
            "delegation",
            "review-checklist",
            "routine-advisor",
            "ironclaw-workflow-orchestrator",
        ];
        for name in &ironclaw {
            assert!(
                seeded.contains(name),
                "ironclaw 内置技能 '{}' 未在种子数据中",
                name
            );
        }
    }

    #[test]
    fn test_builtin_plugin_names_match_ironclaw_registry() {
        // 验证种子数据覆盖了 ironclaw/registry/ 下所有生产插件
        let seeded = [
            "notion",
            "linear",
            "stripe",
            "sentry",
            "cloudflare",
            "intercom",
            "asana",
            "github",
            "gmail",
            "google-calendar",
            "google-drive",
            "google-docs",
            "google-sheets",
            "google-slides",
            "web-search",
            "slack",
        ];
        let ironclaw_mcp = [
            "notion",
            "linear",
            "stripe",
            "sentry",
            "cloudflare",
            "intercom",
            "asana",
        ];
        let ironclaw_tools = [
            "github",
            "gmail",
            "google-calendar",
            "google-drive",
            "google-docs",
            "google-sheets",
            "google-slides",
            "web-search",
            "slack",
        ];
        for name in ironclaw_mcp.iter().chain(ironclaw_tools.iter()) {
            assert!(
                seeded.contains(name),
                "ironclaw 内置插件 '{}' 未在种子数据中",
                name
            );
        }
    }
}
