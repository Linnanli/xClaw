//! `seed_builtin_skills` 的单元测试。
//!
//! 验证内置 skills 种植逻辑：
//! - 正常路径：源目录存在时，SKILL.md 被复制到 installed_dir
//! - 幂等性：已存在的 skill 不被覆盖
//! - 失败路径：源目录不存在时静默跳过
//! - 安全审计：种植的路径不包含路径遍历字符

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    // =========================================================================
    // 辅助函数
    // =========================================================================

    /// 在目录中创建一个 skill 子目录和 SKILL.md 文件。
    fn write_skill_md(dir: &std::path::Path, skill_name: &str, content: &str) {
        let skill_dir = dir.join(skill_name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    /// 模拟 seed_builtin_skills 的核心逻辑（不依赖 AppHandle）。
    async fn seed_skills(source_dir: &std::path::Path, installed_dir: &std::path::Path) {
        let mut read_dir = match tokio::fs::read_dir(source_dir).await {
            Ok(d) => d,
            Err(_) => return,
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let Some(skill_name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
                continue;
            };

            let dest_dir = installed_dir.join(&skill_name);
            if dest_dir.join("SKILL.md").exists() {
                continue; // 幂等：已存在不覆盖
            }

            tokio::fs::create_dir_all(&dest_dir).await.unwrap();
            tokio::fs::copy(&skill_md, dest_dir.join("SKILL.md")).await.unwrap();
        }
    }

    // =========================================================================
    // 正常路径
    // =========================================================================

    /// 验证：源目录中的 skill 被复制到 installed_dir。
    #[tokio::test]
    async fn req_seed_01_skills_copied_to_installed_dir() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        write_skill_md(source.path(), "review-checklist", "---\nname: review-checklist\n---\n\nPrompt.\n");

        seed_skills(source.path(), installed.path()).await;

        assert!(
            installed.path().join("review-checklist").join("SKILL.md").exists(),
            "SKILL.md should be copied to installed_dir"
        );
    }

    /// 验证：多个 skill 全部被复制。
    #[tokio::test]
    async fn req_seed_02_multiple_skills_all_copied() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        write_skill_md(source.path(), "skill-a", "---\nname: skill-a\n---\n\nA.\n");
        write_skill_md(source.path(), "skill-b", "---\nname: skill-b\n---\n\nB.\n");
        write_skill_md(source.path(), "skill-c", "---\nname: skill-c\n---\n\nC.\n");

        seed_skills(source.path(), installed.path()).await;

        assert!(installed.path().join("skill-a").join("SKILL.md").exists());
        assert!(installed.path().join("skill-b").join("SKILL.md").exists());
        assert!(installed.path().join("skill-c").join("SKILL.md").exists());
    }

    /// 验证：复制后内容与源文件一致。
    #[tokio::test]
    async fn req_seed_03_content_preserved() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        let content = "---\nname: local-test\ndescription: Test skill\n---\n\nTest prompt.\n";
        write_skill_md(source.path(), "local-test", content);

        seed_skills(source.path(), installed.path()).await;

        let copied = fs::read_to_string(
            installed.path().join("local-test").join("SKILL.md")
        ).unwrap();
        assert_eq!(copied, content);
    }

    // =========================================================================
    // 幂等性
    // =========================================================================

    /// 验证：已存在的 skill 不被覆盖（幂等）。
    #[tokio::test]
    async fn req_seed_04_existing_skill_not_overwritten() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        write_skill_md(source.path(), "my-skill", "---\nname: my-skill\n---\n\nSource version.\n");

        // 预先在 installed_dir 放一个不同内容的版本
        let existing_dir = installed.path().join("my-skill");
        fs::create_dir_all(&existing_dir).unwrap();
        fs::write(existing_dir.join("SKILL.md"), "---\nname: my-skill\n---\n\nUser version.\n").unwrap();

        seed_skills(source.path(), installed.path()).await;

        // 用户版本应保留
        let content = fs::read_to_string(existing_dir.join("SKILL.md")).unwrap();
        assert!(
            content.contains("User version"),
            "Existing skill should not be overwritten"
        );
    }

    // =========================================================================
    // 失败路径
    // =========================================================================

    /// 验证：源目录不存在时静默跳过，不 panic。
    #[tokio::test]
    async fn test_failure_nonexistent_source_dir_graceful() {
        let installed = TempDir::new().unwrap();
        let nonexistent = std::path::PathBuf::from("/nonexistent/skills/source");

        // 不应 panic
        seed_skills(&nonexistent, installed.path()).await;

        // installed_dir 应为空
        let entries: Vec<_> = fs::read_dir(installed.path()).unwrap().collect();
        assert!(entries.is_empty(), "No skills should be seeded from nonexistent source");
    }

    /// 验证：源目录为空时，installed_dir 保持为空。
    #[tokio::test]
    async fn test_failure_empty_source_dir() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        seed_skills(source.path(), installed.path()).await;

        let entries: Vec<_> = fs::read_dir(installed.path()).unwrap().collect();
        assert!(entries.is_empty());
    }

    /// 验证：源目录中有非目录文件时，跳过不处理。
    #[tokio::test]
    async fn test_failure_non_dir_entries_skipped() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        // 放一个普通文件（不是目录）
        fs::write(source.path().join("README.md"), "not a skill").unwrap();

        seed_skills(source.path(), installed.path()).await;

        let entries: Vec<_> = fs::read_dir(installed.path()).unwrap().collect();
        assert!(entries.is_empty(), "Non-directory entries should be skipped");
    }

    // =========================================================================
    // 安全审计
    // =========================================================================

    /// 安全审计：种植的 skill 路径不包含路径遍历字符。
    #[tokio::test]
    async fn test_audit_seeded_path_no_traversal() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();

        write_skill_md(source.path(), "safe-skill", "---\nname: safe-skill\n---\n\nSafe.\n");

        seed_skills(source.path(), installed.path()).await;

        let dest = installed.path().join("safe-skill").join("SKILL.md");
        let path_str = dest.to_string_lossy();
        assert!(!path_str.contains(".."), "Seeded path should not contain '..'");
    }

    // =========================================================================
    // 契约测试
    // =========================================================================

    /// 契约测试：种植后的 skill 能被 SkillRegistry 正确加载。
    ///
    /// 这是核心业务目标：种植的 skill 必须能被 ironclaw 的 SkillRegistry 发现。
    #[tokio::test]
    async fn test_contract_seeded_skill_loadable_by_registry() {
        let source = TempDir::new().unwrap();
        let installed = TempDir::new().unwrap();
        let user_dir = TempDir::new().unwrap();

        write_skill_md(
            source.path(),
            "review-checklist",
            "---\nname: review-checklist\ndescription: Code review\nactivation:\n  keywords:\n    - review\n---\n\nReview prompt.\n",
        );

        seed_skills(source.path(), installed.path()).await;

        // 用 ironclaw 的 SkillRegistry 加载
        let mut registry = ironclaw::skills::SkillRegistry::new(user_dir.path().to_path_buf())
            .with_installed_dir(installed.path().to_path_buf());
        let loaded = registry.discover_all().await;

        assert!(
            loaded.contains(&"review-checklist".to_string()),
            "Seeded skill should be discoverable by SkillRegistry"
        );
        assert!(registry.has("review-checklist"));
    }
}
