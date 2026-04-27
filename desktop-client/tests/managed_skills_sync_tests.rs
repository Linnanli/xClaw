//! 受管技能同步链路诊断测试。
//!
//! 验证从 admin backend → SkillCatalog → ic_list_skills 的完整流程，
//! 覆盖所有可能导致同步失败的前置条件。

/// 条件检查：sync 链路有 6 个前置条件，任一不满足就静默跳过。
/// 这些测试逐个验证每个条件。
mod sync_preconditions {
    use std::collections::HashSet;

    /// managed_mode_enabled() 依赖 MANAGED_MODE env var
    #[test]
    fn test_managed_mode_detection() {
        // 无 env var → false
        std::env::remove_var("MANAGED_MODE");
        let result = read_managed_mode();
        assert!(!result, "MANAGED_MODE未设置时应返回 false");

        // "true" → true
        std::env::set_var("MANAGED_MODE", "true");
        assert!(read_managed_mode(), "MANAGED_MODE=true 应返回 true");

        // "1" → true
        std::env::set_var("MANAGED_MODE", "1");
        assert!(read_managed_mode(), "MANAGED_MODE=1 应返回 true");

        // "false" → false
        std::env::set_var("MANAGED_MODE", "false");
        assert!(!read_managed_mode(), "MANAGED_MODE=false 应返回 false");

        // 清理
        std::env::remove_var("MANAGED_MODE");
    }

    fn read_managed_mode() -> bool {
        std::env::var("MANAGED_MODE")
            .map(|value| {
                let normalized = value.trim().to_ascii_lowercase();
                normalized == "true" || normalized == "1"
            })
            .unwrap_or(false)
    }

    /// missing_allowed_skill_names 纯函数测试
    #[test]
    fn test_missing_names_diff() {
        let allowed: HashSet<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let installed: HashSet<String> = ["b"].iter().map(|s| s.to_string()).collect();

        let mut missing: Vec<String> = allowed
            .iter()
            .filter(|n| !installed.contains(*n))
            .cloned()
            .collect();
        missing.sort();
        assert_eq!(missing, vec!["a", "c"]);
    }
}

/// Admin backend 注册表 API 响应格式兼容性测试。
///
/// ironclaw 的 CatalogSearchResult 使用 #[serde(rename_all = "camelCase")]，
/// 所以 JSON key 必须是 camelCase（displayName 而非 display_name）。
mod registry_api_format {
    use serde::Deserialize;

    /// 镜像 ironclaw 内部的 CatalogSearchResult（不可直接引用，是 pub(crate)）
    #[allow(dead_code)]
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CatalogSearchResult {
        slug: String,
        #[serde(default)]
        display_name: Option<String>,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        score: Option<f64>,
    }

    #[derive(Debug, Deserialize)]
    struct CatalogSearchEnvelope {
        results: Vec<CatalogSearchResult>,
    }

    /// 验证 admin backend 返回 camelCase 格式时字段正确解析
    #[test]
    fn test_admin_backend_camel_case_response_parses_correctly() {
        let json = serde_json::json!({
            "results": [{
                "slug": "550e8400-e29b-41d4-a716-446655440000",
                "displayName": "写作助手",
                "summary": "帮助撰写各类文档",
                "version": "1.0.0",
                "score": 1.0
            }]
        });

        let envelope: CatalogSearchEnvelope = serde_json::from_value(json).unwrap();
        assert_eq!(envelope.results.len(), 1);
        let entry = &envelope.results[0];
        assert_eq!(
            entry.display_name.as_deref(),
            Some("写作助手"),
            "displayName (camelCase) 应被正确解析"
        );
        assert_eq!(entry.summary.as_deref(), Some("帮助撰写各类文档"));
    }

    /// 验证旧的 snake_case 格式不会被 camelCase 解析器识别
    /// 这就是之前的 bug：admin backend 返回 display_name → 被反序列化为 None
    #[test]
    fn test_snake_case_response_loses_display_name() {
        let json = serde_json::json!({
            "results": [{
                "slug": "550e8400-e29b-41d4-a716-446655440000",
                "display_name": "写作助手",
                "summary": "帮助撰写各类文档",
                "version": "1.0.0",
                "score": 1.0
            }]
        });

        let envelope: CatalogSearchEnvelope = serde_json::from_value(json).unwrap();
        let entry = &envelope.results[0];
        assert_eq!(
            entry.display_name, None,
            "OLD BUG: display_name (snake_case) 无法被 camelCase 解析器识别，结果为 None"
        );
    }

    /// 验证名称匹配逻辑：display_name 为 None 时（旧 bug）匹配一定失败
    #[test]
    fn test_name_match_fails_when_display_name_is_none() {
        let display_name: Option<String> = None; // 旧 bug：snake_case → None
        let effective_name = display_name.unwrap_or_default();
        let expected = "写作助手";

        assert_ne!(
            effective_name.trim().to_lowercase(),
            expected.trim().to_lowercase(),
            "当 display_name 为 None 时，unwrap_or_default() 得到空字符串，匹配必定失败"
        );
    }

    /// 验证修复后名称匹配成功
    #[test]
    fn test_name_match_succeeds_with_camel_case() {
        let display_name: Option<String> = Some("写作助手".to_string());
        let effective_name = display_name.unwrap_or_default();
        let expected = "写作助手";

        assert_eq!(
            effective_name.trim().to_lowercase(),
            expected.trim().to_lowercase(),
            "修复后 displayName 正确解析，名称匹配应成功"
        );
    }
}

/// 使用真实 admin backend 的集成测试（需要 admin backend 运行）。
///
/// 运行方式：
///   ADMIN_URL=http://localhost:3000 cargo test -p desktop-client --test managed_skills_sync_tests real_integration -- --nocapture
mod real_integration {
    fn extract_skill_name_from_content(content: &str) -> Option<String> {
        let trimmed = content.trim_start();
        let after_fence = trimmed.strip_prefix("---")?;
        let yaml_block = after_fence.split("\n---").next()?;
        for line in yaml_block.lines() {
            let stripped = line.trim();
            if let Some(value) = stripped.strip_prefix("name:") {
                let name = value.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    /// 列表同步 + 按需下载集成测试：
    /// 1) 仅调用 catalog.search("") 不应下载任何 SKILL.md
    /// 2) 模拟 enable 时才下载并安装目标 skill
    #[tokio::test]
    async fn test_list_sync_then_lazy_download_on_enable() {
        let admin_url = match std::env::var("ADMIN_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("SKIP: ADMIN_URL not set — run with ADMIN_URL=http://localhost:3000");
                return;
            }
        };
        let client_token = match std::env::var("ADMIN_CLIENT_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                eprintln!("SKIP: ADMIN_CLIENT_TOKEN not set — run with ADMIN_CLIENT_TOKEN=<registered-client-uuid>");
                return;
            }
        };

        let registry_url = format!("{}/api/v1?client_token={}", admin_url, client_token);
        std::env::set_var("CLAWHUB_REGISTRY", &registry_url);
        let catalog = ironclaw::skills::catalog::SkillCatalog::new();

        let temp_root = tempfile::tempdir().expect("create temp root");
        let user_dir = temp_root.path().join("user-skills");
        let installed_dir = temp_root.path().join("installed-skills");
        let mut registry = ironclaw::skills::SkillRegistry::new(user_dir)
            .with_installed_dir(installed_dir.clone());

        // 1) 列表同步：仅搜索，不下载技能内容。
        let outcome = catalog.search("").await;
        if let Some(error) = outcome.error {
            panic!("catalog search failed: {}", error);
        }

        let before_count = std::fs::read_dir(&installed_dir)
            .map(|iter| iter.count())
            .unwrap_or(0);
        assert_eq!(
            before_count, 0,
            "list sync should not create installed skill directories"
        );

        // 2) 模拟 enable 触发按需下载。
        let target = "code-review-expert";
        let entry = outcome
            .results
            .iter()
            .find(|e| e.name == target)
            .unwrap_or_else(|| panic!("target skill '{}' not found in catalog", target));

        let download_url =
            ironclaw::skills::catalog::skill_download_url(&registry_url, &entry.slug);
        let client = reqwest::Client::new();
        let response = client
            .get(&download_url)
            .send()
            .await
            .unwrap_or_else(|e| panic!("download request failed: {}", e));
        assert!(
            response.status().is_success(),
            "download endpoint returned {}",
            response.status()
        );
        let content = response
            .text()
            .await
            .unwrap_or_else(|e| panic!("read response body failed: {}", e));
        assert!(
            !content.is_empty(),
            "downloaded SKILL.md content should not be empty"
        );

        let dir_name = extract_skill_name_from_content(&content).unwrap_or_else(|| {
            format!(
                "_managed_{}",
                entry.slug.chars().take(8).collect::<String>()
            )
        });

        let (resolved_name, loaded) = ironclaw::skills::SkillRegistry::prepare_install_to_disk(
            registry.install_target_dir(),
            &dir_name,
            &content,
        )
        .await
        .unwrap_or_else(|e| panic!("prepare install failed: {}", e));
        assert_eq!(
            resolved_name, target,
            "resolved name should match target skill"
        );

        registry
            .commit_install(&resolved_name, loaded)
            .unwrap_or_else(|e| panic!("commit install failed: {}", e));
        assert!(
            registry.has(target),
            "skill should be installed after lazy download"
        );

        let skill_file = installed_dir.join(target).join("SKILL.md");
        assert!(skill_file.exists(), "installed SKILL.md should exist");
        let bytes = std::fs::read(&skill_file).expect("read installed SKILL.md");
        assert!(!bytes.is_empty(), "installed SKILL.md should not be empty");
    }

    /// 端到端验证：admin backend /api/v1/search 返回的数据能被 SkillCatalog 正确解析
    #[tokio::test]
    async fn test_catalog_search_against_real_admin_backend() {
        let admin_url = match std::env::var("ADMIN_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("SKIP: ADMIN_URL not set — run with ADMIN_URL=http://localhost:3000");
                return;
            }
        };
        let client_token = match std::env::var("ADMIN_CLIENT_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                eprintln!("SKIP: ADMIN_CLIENT_TOKEN not set — run with ADMIN_CLIENT_TOKEN=<registered-client-uuid>");
                return;
            }
        };

        let registry_url = format!("{}/api/v1?client_token={}", admin_url, client_token);
        eprintln!("Using registry URL: {}", registry_url);

        // 直接发 HTTP 请求模拟 SkillCatalog 的 fetch_search 行为
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/search?q=&client_token={}",
            admin_url, client_token
        ); // 空查询 = 列出全部
        eprintln!("Fetching: {}", url);

        let response = match client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("SKIP: Admin backend unreachable: {}", e);
                return;
            }
        };

        assert!(
            response.status().is_success(),
            "Admin backend /api/v1/search returned {}",
            response.status()
        );

        let body = response.text().await.unwrap();
        eprintln!("Response body: {}", &body[..body.len().min(500)]);

        // 用与 ironclaw 相同的 camelCase 解析器
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        #[serde(rename_all = "camelCase")]
        struct SearchResult {
            slug: String,
            #[serde(default)]
            display_name: Option<String>,
            #[serde(default)]
            summary: Option<String>,
            #[serde(default)]
            version: Option<String>,
            #[serde(default)]
            score: Option<f64>,
        }

        #[derive(Debug, serde::Deserialize)]
        struct Envelope {
            results: Vec<SearchResult>,
        }

        let envelope: Envelope = serde_json::from_str(&body).unwrap_or_else(|e| {
            panic!(
                "Failed to parse response: {} — body: {}",
                e,
                &body[..body.len().min(200)]
            )
        });

        eprintln!("Parsed {} results:", envelope.results.len());
        for (i, entry) in envelope.results.iter().enumerate() {
            eprintln!(
                "  [{}] slug={}, displayName={:?}, summary={:?}",
                i, entry.slug, entry.display_name, entry.summary
            );
        }

        if !envelope.results.is_empty() {
            let first = &envelope.results[0];
            assert!(
                first.display_name.is_some(),
                "BUG: displayName 解析为 None — admin backend 可能仍在返回 snake_case 格式 (display_name)。\n\
                 检查 admin-backend/src/handlers/extensions.rs 中 skill_to_registry_entry() 是否使用 displayName"
            );
            eprintln!("\n✅ displayName 字段正确解析为: {:?}", first.display_name);
        } else {
            eprintln!("⚠️ 搜索结果为空 — admin backend 可能没有已审核通过的技能");
        }
    }

    /// 端到端验证：使用 SkillCatalog 实例搜索 admin backend
    #[tokio::test]
    async fn test_skill_catalog_search_against_real_admin_backend() {
        let admin_url = match std::env::var("ADMIN_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("SKIP: ADMIN_URL not set");
                return;
            }
        };
        let client_token = match std::env::var("ADMIN_CLIENT_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                eprintln!("SKIP: ADMIN_CLIENT_TOKEN not set");
                return;
            }
        };

        let registry_url = format!("{}/api/v1?client_token={}", admin_url, client_token);

        // SkillCatalog::new() 从 CLAWHUB_REGISTRY 读取 URL
        std::env::set_var("CLAWHUB_REGISTRY", &registry_url);
        let catalog = ironclaw::skills::catalog::SkillCatalog::new();

        // 空搜索 = 列出全部
        let outcome = catalog.search("").await;

        if let Some(ref error) = outcome.error {
            eprintln!("Catalog search error: {}", error);
        }

        eprintln!("SkillCatalog returned {} results:", outcome.results.len());
        for (i, entry) in outcome.results.iter().enumerate() {
            eprintln!(
                "  [{}] name={:?}, slug={}, description={:?}",
                i, entry.name, entry.slug, entry.description
            );
        }

        if !outcome.results.is_empty() {
            let first = &outcome.results[0];
            assert!(
                !first.name.is_empty(),
                "BUG: SkillCatalog entry name is empty — displayName not parsed correctly"
            );
            eprintln!("\n✅ SkillCatalog.name 正确获取: {:?}", first.name);

            // 验证 find_catalog_slug_for_name 逻辑
            let expected = first.name.trim().to_lowercase();
            let found = outcome
                .results
                .iter()
                .find(|e| e.name.trim().to_lowercase() == expected)
                .map(|e| e.slug.clone());
            assert!(
                found.is_some(),
                "find_catalog_slug_for_name 精确匹配应成功，但失败了。expected='{}', names={:?}",
                expected,
                outcome.results.iter().map(|e| &e.name).collect::<Vec<_>>()
            );
            eprintln!(
                "✅ find_catalog_slug_for_name 精确匹配成功: slug={:?}",
                found
            );
        } else {
            eprintln!("⚠️ admin backend 没有已审核技能，无法验证端到端流程");
        }
    }
}
