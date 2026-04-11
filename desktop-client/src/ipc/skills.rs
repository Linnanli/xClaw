//! 技能管理 Tauri Commands。
//!
//! 直接调用 `SkillRegistry` 和 `SkillCatalog` 管理技能。
//!
//! # 注意
//!
//! `SkillRegistry` 使用 `std::sync::RwLock`，其 guard 不是 `Send`。
//! 因此不能在持有 guard 的同时 `.await`。需要使用 split 方法：
//! - install: `prepare_install_to_disk()` (无锁) → `commit_install()` (短锁)
//! - uninstall: `validate_remove()` (短锁) → `delete_skill_files()` (无锁) → `commit_remove()` (短锁)

use serde::{Deserialize, Serialize};
use tauri::State;

use super::persistence::persist_disabled_items;
use crate::state::{AppState, EngineState};

const DISABLED_SKILLS_SETTING_KEY: &str = "desktop_disabled_skills";

/// 技能信息（前端展示用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    /// 技能来源：`"workspace"` | `"user"` | `"installed"`
    pub source: String,
    /// 信任级别：`"trusted"` | `"installed"`
    pub trust: String,
    /// 激活关键词列表（供前端展示触发条件）
    pub keywords: Vec<String>,
    /// 是否启用（由 desktop-client 本地设置控制）。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// 技能目录搜索结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSearchResult {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub version: String,
    pub score: f64,
}

/// 列出已安装技能。
#[tauri::command]
pub async fn ic_list_skills(state: State<'_, EngineState>) -> Result<Vec<SkillInfo>, String> {
    let state = state.get()?;
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    let guard = registry
        .read()
        .map_err(|e| format!("Lock poisoned: {}", e))?;

    let result = guard
        .skills()
        .iter()
        .map(|s| {
            let source = match &s.source {
                ironclaw::skills::SkillSource::Workspace(_)
                | ironclaw::skills::SkillSource::Bundled(_) => "workspace",
                ironclaw::skills::SkillSource::User(_) => "user",
            };
            let trust = match s.trust {
                ironclaw::skills::SkillTrust::Trusted => "trusted",
                ironclaw::skills::SkillTrust::Installed => "installed",
            };
            SkillInfo {
                name: s.manifest.name.clone(),
                version: s.manifest.version.clone(),
                description: s.manifest.description.clone(),
                source: source.to_string(),
                trust: trust.to_string(),
                keywords: s
                    .manifest
                    .activation
                    .keywords
                    .iter()
                    .take(5)
                    .cloned()
                    .collect(),
                enabled: state.skill_enabled(&s.manifest.name),
            }
        })
        .collect();

    Ok(result)
}

#[tauri::command]
pub async fn ic_enable_skill(
    state: State<'_, EngineState>,
    name: String,
) -> Result<(), String> {
    let state = state.get()?;
    ensure_skill_exists(state, &name)?;
    set_skill_enabled_with_persist(state, &name, true).await?;
    tracing::info!(skill = %name, "Skill enabled");
    Ok(())
}

#[tauri::command]
pub async fn ic_disable_skill(
    state: State<'_, EngineState>,
    name: String,
) -> Result<(), String> {
    let state = state.get()?;
    ensure_skill_exists(state, &name)?;
    set_skill_enabled_with_persist(state, &name, false).await?;
    tracing::info!(skill = %name, "Skill disabled");
    Ok(())
}

/// 搜索技能目录。
#[tauri::command]
pub async fn ic_search_skills(
    state: State<'_, EngineState>,
    query: String,
) -> Result<Vec<CatalogSearchResult>, String> {
    let state = state.get()?;
    let catalog = state
        .skill_catalog
        .as_ref()
        .ok_or("Skill catalog not available")?;

    let outcome = catalog.search(&query).await;

    Ok(outcome
        .results
        .into_iter()
        .map(|e| CatalogSearchResult {
            name: e.name,
            slug: e.slug,
            description: e.description,
            version: e.version,
            score: e.score,
        })
        .collect())
}

/// 安装技能（从 SKILL.md 内容安装）。
///
/// 使用 split 方法避免跨 await 持有 RwLock guard：
/// 1. `prepare_install_to_disk()` — 写入文件系统（无锁，可 await）
/// 2. `commit_install()` — 更新内存注册表（短锁，同步）
#[tauri::command]
pub async fn ic_install_skill(
    state: State<'_, EngineState>,
    content: String,
) -> Result<String, String> {
    let state = state.get()?;
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    // 获取安装目标目录（短锁，立即释放）
    let install_dir = {
        let guard = registry
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard.install_target_dir().to_path_buf()
    };

    // Phase 1: 写入文件系统（无锁，可 await）
    let (name, loaded) = ironclaw::skills::SkillRegistry::prepare_install_to_disk(
        &install_dir,
        "_pending",
        &content,
    )
    .await
    .map_err(|e| format!("Failed to install skill: {}", e))?;

    // Phase 2: 更新内存注册表（短锁，同步）
    {
        let mut guard = registry
            .write()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard
            .commit_install(&name, loaded)
            .map_err(|e| format!("Failed to commit install: {}", e))?;
    }

    tracing::info!(skill = %name, "Skill installed");
    Ok(name)
}

/// 卸载技能。
///
/// 使用 split 方法避免跨 await 持有 RwLock guard：
/// 1. `validate_remove()` — 验证并获取路径（短锁，同步）
/// 2. `delete_skill_files()` — 删除文件（无锁，可 await）
/// 3. `commit_remove()` — 更新内存注册表（短锁，同步）
#[tauri::command]
pub async fn ic_uninstall_skill(state: State<'_, EngineState>, name: String) -> Result<(), String> {
    let state = state.get()?;
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    // Phase 1: 验证并获取路径（短锁，同步）
    let path = {
        let guard = registry
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard
            .validate_remove(&name)
            .map_err(|e| format!("Failed to validate removal: {}", e))?
    };

    // Phase 2: 删除文件（无锁，可 await）
    ironclaw::skills::SkillRegistry::delete_skill_files(&path)
        .await
        .map_err(|e| format!("Failed to delete skill files: {}", e))?;

    // Phase 3: 更新内存注册表（短锁，同步）
    {
        let mut guard = registry
            .write()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard
            .commit_remove(&name)
            .map_err(|e| format!("Failed to commit removal: {}", e))?;
    }

    tracing::info!(skill = %name, "Skill uninstalled");
    Ok(())
}

fn ensure_skill_exists(state: &AppState, name: &str) -> Result<(), String> {
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;
    let guard = registry
        .read()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    if guard.has(name) {
        return Ok(());
    }
    Err(format!("Skill not found: {}", name))
}

async fn persist_disabled_skills(state: &AppState) -> Result<(), String> {
    let disabled = state.disabled_skills_snapshot()?;
    persist_disabled_items(state, DISABLED_SKILLS_SETTING_KEY, disabled, "skills").await
}

async fn set_skill_enabled_with_persist(
    state: &AppState,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    state.set_skill_enabled(name, enabled)?;
    if let Err(error) = persist_disabled_skills(state).await {
        let rollback_error = state
            .set_skill_enabled(name, !enabled)
            .err()
            .unwrap_or_default();
        if rollback_error.is_empty() {
            return Err(error);
        }
        return Err(format!(
            "{}; rollback failed: {}",
            error, rollback_error
        ));
    }
    Ok(())
}
