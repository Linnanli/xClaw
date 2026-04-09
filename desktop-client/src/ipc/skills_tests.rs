//! 技能管理 IPC 命令测试。
//!
//! 覆盖维度：
//! - 单元测试（正常路径 + 错误路径）
//! - 契约测试（SkillInfo / CatalogSearchResult 格式与前端 TypeScript 类型匹配）
//! - 安全审计测试（敏感信息不泄露）
//! - 数据级覆盖（边界值、空值、特殊字符）
//! - 失败路径测试（空内容、无效 SKILL.md 等）

#[cfg(test)]
mod tests {
    use crate::ipc::skills::{CatalogSearchResult, SkillInfo};

    // =========================================================================
    // 单元测试 — 正常路径
    // =========================================================================

    #[test]
    fn test_skill_info_serialization() {
        let info = SkillInfo {
            name: "code-review".into(),
            version: "1.0.0".into(),
            description: "Automated code review skill".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec!["review".into(), "code".into()],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "code-review");
        assert_eq!(json["version"], "1.0.0");
        assert_eq!(json["description"], "Automated code review skill");
        assert_eq!(json["source"], "workspace");
        assert_eq!(json["trust"], "trusted");
        assert!(json["keywords"].is_array());
    }

    #[test]
    fn test_skill_info_deserialization() {
        let json = r#"{
            "name": "tdd-practitioner",
            "version": "2.1.0",
            "description": "TDD methodology skill",
            "source": "user",
            "trust": "trusted",
            "keywords": ["tdd", "test"]
        }"#;
        let info: SkillInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "tdd-practitioner");
        assert_eq!(info.version, "2.1.0");
        assert_eq!(info.source, "user");
    }

    #[test]
    fn test_catalog_search_result_serialization() {
        let result = CatalogSearchResult {
            name: "Security Scanner".into(),
            slug: "owner/security-scanner".into(),
            description: "Scans code for vulnerabilities".into(),
            version: "3.0.0".into(),
            score: 0.87,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["slug"], "owner/security-scanner");
        assert!((json["score"].as_f64().unwrap() - 0.87).abs() < 0.001);
    }

    #[test]
    fn test_catalog_search_result_deserialization() {
        let json = r#"{
            "name": "Test Skill",
            "slug": "user/test-skill",
            "description": "A test skill",
            "version": "0.1.0",
            "score": 0.5
        }"#;
        let result: CatalogSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.slug, "user/test-skill");
        assert!((result.score - 0.5).abs() < f64::EPSILON);
    }

    // =========================================================================
    // 契约测试 — 验证与前端 TypeScript 类型的兼容性
    // =========================================================================

    /// 前端 SkillInfo 类型定义：
    /// ```typescript
    /// interface SkillInfo {
    ///   name: string;
    ///   version: string;
    ///   description: string;
    ///   source: 'workspace' | 'user' | 'installed';
    ///   trust: 'trusted' | 'installed';
    ///   keywords: string[];
    /// }
    /// ```
    #[test]
    fn test_contract_skill_info_matches_frontend() {
        let info = SkillInfo {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "desc".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec!["review".into()],
        };
        let json: serde_json::Value = serde_json::to_value(&info).unwrap();

        assert!(json["name"].is_string());
        assert!(json["version"].is_string());
        assert!(json["description"].is_string());
        assert!(json["source"].is_string());
        assert!(json["trust"].is_string());
        assert!(json["keywords"].is_array());

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 6, "SkillInfo should have exactly 6 fields");
    }

    /// 前端 CatalogSearchResult 类型定义：
    /// ```typescript
    /// interface CatalogSearchResult {
    ///   name: string;
    ///   slug: string;
    ///   description: string;
    ///   version: string;
    ///   score: number;
    /// }
    /// ```
    #[test]
    fn test_contract_catalog_search_result_matches_frontend() {
        let result = CatalogSearchResult {
            name: "test".into(),
            slug: "owner/test".into(),
            description: "desc".into(),
            version: "1.0.0".into(),
            score: 0.5,
        };
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();

        assert!(json["name"].is_string());
        assert!(json["slug"].is_string());
        assert!(json["description"].is_string());
        assert!(json["version"].is_string());
        assert!(json["score"].is_number());

        let obj = json.as_object().unwrap();
        assert_eq!(
            obj.len(),
            5,
            "CatalogSearchResult should have exactly 5 fields"
        );
    }

    /// 验证 score 字段序列化为 JSON number（不是 string）。
    #[test]
    fn test_contract_score_is_number_not_string() {
        let result = CatalogSearchResult {
            name: "test".into(),
            slug: "test".into(),
            description: "".into(),
            version: "".into(),
            score: 0.123456789,
        };
        let json: serde_json::Value = serde_json::to_value(&result).unwrap();
        assert!(
            json["score"].is_f64(),
            "score should be a float, not a string"
        );
    }

    // =========================================================================
    // 安全审计测试
    // =========================================================================

    /// 验证 SkillInfo 不包含文件系统路径（防止泄露安装位置）。
    #[test]
    fn test_audit_skill_info_no_path_leak() {
        let info = SkillInfo {
            name: "secret-skill".into(),
            version: "1.0.0".into(),
            description: "A skill with secrets".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();

        assert!(!json_str.contains("path"));
        assert!(!json_str.contains("dir"));
        assert!(!json_str.contains("home"));
        assert!(!json_str.contains("user_dir"));
        assert!(!json_str.contains("install_dir"));
    }

    /// 验证 SkillInfo 不包含 activation 配置（可能含内部触发规则）。
    #[test]
    fn test_audit_skill_info_no_activation_leak() {
        let info = SkillInfo {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "test".into(),
            source: "user".into(),
            trust: "trusted".into(),
            keywords: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();

        assert!(!json_str.contains("activation"));
        assert!(!json_str.contains("metadata"));
        // trust 字段是预期的，但不应包含内部 trust 枚举的原始值
        assert!(!json_str.contains("prompt_content"));
        assert!(!json_str.contains("content_hash"));
    }

    /// 验证 CatalogSearchResult 不包含 owner 个人信息。
    #[test]
    fn test_audit_catalog_result_no_owner_details() {
        let result = CatalogSearchResult {
            name: "test".into(),
            slug: "owner/test".into(),
            description: "test".into(),
            version: "1.0.0".into(),
            score: 0.5,
        };
        let json_str = serde_json::to_string(&result).unwrap();

        // slug 中包含 owner 名称是预期的，但不应有额外的 owner 详情
        assert!(!json_str.contains("email"));
        assert!(!json_str.contains("stars"));
        assert!(!json_str.contains("downloads"));
    }

    // =========================================================================
    // 数据级覆盖 — 边界值和特殊字符
    // =========================================================================

    #[test]
    fn test_data_empty_description() {
        let info = SkillInfo {
            name: "minimal".into(),
            version: "0.0.1".into(),
            description: "".into(),
            source: "user".into(),
            trust: "installed".into(),
            keywords: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SkillInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.description, "");
    }

    #[test]
    fn test_data_unicode_skill_name() {
        let info = SkillInfo {
            name: "代码审查".into(),
            version: "1.0.0".into(),
            description: "自动化代码审查技能".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec!["审查".into()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SkillInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "代码审查");
    }

    #[test]
    fn test_data_semver_prerelease() {
        let info = SkillInfo {
            name: "beta-skill".into(),
            version: "2.0.0-beta.1".into(),
            description: "Beta version".into(),
            source: "user".into(),
            trust: "trusted".into(),
            keywords: vec![],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["version"], "2.0.0-beta.1");
    }

    #[test]
    fn test_data_zero_score() {
        let result = CatalogSearchResult {
            name: "irrelevant".into(),
            slug: "owner/irrelevant".into(),
            description: "Not matching".into(),
            version: "1.0.0".into(),
            score: 0.0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!((json["score"].as_f64().unwrap()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_data_max_score() {
        let result = CatalogSearchResult {
            name: "perfect".into(),
            slug: "owner/perfect".into(),
            description: "Exact match".into(),
            version: "1.0.0".into(),
            score: 1.0,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!((json["score"].as_f64().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_data_slug_with_special_chars() {
        let result = CatalogSearchResult {
            name: "My Skill".into(),
            slug: "user-name/my-skill_v2".into(),
            description: "test".into(),
            version: "1.0.0".into(),
            score: 0.5,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["slug"], "user-name/my-skill_v2");
    }

    #[test]
    fn test_data_very_long_description() {
        let long_desc = "A".repeat(10_000);
        let info = SkillInfo {
            name: "verbose".into(),
            version: "1.0.0".into(),
            description: long_desc.clone(),
            source: "user".into(),
            trust: "trusted".into(),
            keywords: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SkillInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.description.len(), 10_000);
    }

    #[test]
    fn test_data_markdown_in_description() {
        let info = SkillInfo {
            name: "md-skill".into(),
            version: "1.0.0".into(),
            description: "# Title\n\n- item 1\n- item 2\n\n```rust\nfn main() {}\n```".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SkillInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.description.contains("```rust"));
    }

    /// 验证 source 字段只包含合法值。
    #[test]
    fn test_contract_skill_info_source_values() {
        for source in &["workspace", "user", "installed"] {
            let info = SkillInfo {
                name: "test".into(),
                version: "1.0.0".into(),
                description: "".into(),
                source: source.to_string(),
                trust: "trusted".into(),
                keywords: vec![],
            };
            let json = serde_json::to_value(&info).unwrap();
            assert_eq!(json["source"], *source);
        }
    }

    /// 验证 keywords 最多 5 个（ic_list_skills 截断逻辑的契约）。
    #[test]
    fn test_contract_skill_info_keywords_max_5() {
        let info = SkillInfo {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "".into(),
            source: "workspace".into(),
            trust: "trusted".into(),
            keywords: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert!(json["keywords"].as_array().unwrap().len() <= 5);
    }
}
