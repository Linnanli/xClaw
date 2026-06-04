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
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

use super::persistence::{load_string_set, persist_disabled_items};
use crate::managed_policy::load_verified_policy_from_store;
use crate::state::{AppState, EngineState};

const DISABLED_SKILLS_SETTING_KEY: &str = "desktop_disabled_skills";
const MANAGED_ALLOWED_SKILLS_SETTING_KEY: &str = "desktop_managed_allowed_skills";

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

fn managed_mode_enabled() -> bool {
    std::env::var("MANAGED_MODE")
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "true" || normalized == "1"
        })
        .unwrap_or(false)
}

fn configured_registry_url() -> Option<String> {
    std::env::var("CLAWHUB_REGISTRY")
        .ok()
        .or_else(|| std::env::var("CLAWDHUB_REGISTRY").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_skill_catalog_registry_policy(
    managed_mode: bool,
    registry_url: Option<&str>,
) -> Result<(), String> {
    if managed_mode && registry_url.is_none() {
        return Err(
            "Managed mode enabled: CLAWHUB_REGISTRY must point to admin registry".to_string(),
        );
    }
    Ok(())
}

fn validate_skill_install_source(managed_mode: bool) -> Result<(), String> {
    if managed_mode {
        return Err(
            "Managed mode enabled: local SKILL.md content installation is not allowed".to_string(),
        );
    }
    Ok(())
}

async fn ensure_skill_allowed_by_policy(state: &AppState, name: &str) -> Result<(), String> {
    let allowed = load_policy_allowed_skill_names(state).await?;
    if skill_allowed_by_policy(name, allowed.as_ref()) {
        return Ok(());
    }

    Err(format!("Client policy does not allow skill '{}'", name))
}

fn filter_skill_infos_by_allowlist(
    skills: Vec<SkillInfo>,
    allowed: Option<&HashSet<String>>,
) -> Vec<SkillInfo> {
    match allowed {
        Some(allowed_names) => skills
            .into_iter()
            .filter(|skill| allowed_names.contains(&skill.name))
            .collect(),
        None => skills,
    }
}

fn loaded_skill_to_info(skill: &ironclaw::skills::LoadedSkill, enabled: bool) -> SkillInfo {
    let source = match &skill.source {
        ironclaw::skills::SkillSource::Workspace(_) | ironclaw::skills::SkillSource::Bundled(_) => {
            "workspace"
        }
        ironclaw::skills::SkillSource::User(_) => "user",
    };

    SkillInfo {
        name: skill.manifest.name.clone(),
        version: skill.manifest.version.clone(),
        description: skill.manifest.description.clone(),
        source: source.to_string(),
        trust: skill.trust.to_string(),
        keywords: skill
            .manifest
            .activation
            .keywords
            .iter()
            .take(5)
            .cloned()
            .collect(),
        enabled,
    }
}

fn skill_allowed_by_policy(name: &str, allowed: Option<&HashSet<String>>) -> bool {
    match allowed {
        Some(allowed_names) => allowed_names.contains(name),
        None => true,
    }
}

fn log_loaded_skill_allowlist(
    context: &'static str,
    source: &'static str,
    skills: &HashSet<String>,
) {
    tracing::info!(
        context,
        source,
        count = skills.len(),
        skills = ?skills,
        "Skill allowlist loaded"
    );
}

async fn load_signed_policy_skill_names(
    db: &Arc<dyn ironclaw::db::Database>,
    scope_id: &str,
    context: &'static str,
) -> Result<Option<HashSet<String>>, String> {
    match load_verified_policy_from_store(db.as_ref(), scope_id).await {
        Ok(Some(policy)) => {
            let skills = policy.allowed_skill_set();
            log_loaded_skill_allowlist(context, "signed_policy", &skills);
            Ok(Some(skills))
        }
        Ok(None) => {
            tracing::info!(context, "No signed policy in store");
            Ok(None)
        }
        Err(error) => {
            tracing::warn!(context, error = %error, "Failed to load signed policy");
            Err(error)
        }
    }
}

async fn load_policy_allowed_skill_names(
    state: &AppState,
) -> Result<Option<HashSet<String>>, String> {
    if let Some(db) = state.db.as_ref() {
        if let Some(skills) =
            load_signed_policy_skill_names(db, &state.scope_id, "load_policy_allowed_skill_names")
                .await?
        {
            return Ok(Some(skills));
        }
    } else {
        tracing::info!(context = "load_policy_allowed_skill_names", "db is None");
    }

    if !managed_mode_enabled() {
        tracing::info!(
            context = "load_policy_allowed_skill_names",
            "no policy source, returning None (no filtering)"
        );
        return Ok(None);
    }

    let skills = load_string_set(state, MANAGED_ALLOWED_SKILLS_SETTING_KEY).await?;
    log_loaded_skill_allowlist(
        "load_policy_allowed_skill_names",
        "managed_setting",
        &skills,
    );
    Ok(Some(skills))
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
pub(crate) async fn list_skills_from_state(state: &AppState) -> Result<Vec<SkillInfo>, String> {
    if let Err(error) = sync_managed_allowed_skills(state).await {
        tracing::warn!(error = %error, "Managed skills sync before list failed");
    }

    let allowed = load_policy_allowed_skill_names(state).await?;
    tracing::info!(
        has_allowlist = allowed.is_some(),
        allowlist_count = allowed.as_ref().map(|a| a.len()).unwrap_or(0),
        "ic_list_skills: policy allowlist resolved"
    );

    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    let guard = registry
        .read()
        .map_err(|e| format!("Lock poisoned: {}", e))?;

    let result = guard
        .skills()
        .iter()
        .map(|skill| loaded_skill_to_info(skill, state.skill_enabled(&skill.manifest.name)))
        .collect();

    let filtered = filter_skill_infos_by_allowlist(result, allowed.as_ref());
    tracing::info!(
        total_installed = guard.skills().len(),
        after_filter = filtered.len(),
        names = ?filtered.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "ic_list_skills: returning filtered skill list"
    );
    Ok(filtered)
}

#[tauri::command]
pub async fn ic_list_skills(state: State<'_, EngineState>) -> Result<Vec<SkillInfo>, String> {
    list_skills_from_state(state.get()?).await
}

/// Run managed skills sync at engine startup.
///
/// Called from `engine.rs` during startup to auto-install missing managed skills
/// without waiting for the user to open the Skills Tab.
pub async fn startup_sync_managed_skills(state: &AppState) -> Result<(), String> {
    sync_managed_allowed_skills(state).await
}

async fn sync_managed_allowed_skills(state: &AppState) -> Result<(), String> {
    if !managed_mode_enabled() {
        tracing::debug!("sync_managed_allowed_skills: skipped (MANAGED_MODE not enabled)");
        return Ok(());
    }

    let Some(catalog) = state.skill_catalog.as_ref() else {
        tracing::warn!("sync_managed_allowed_skills: skipped (skill_catalog is None)");
        return Ok(());
    };
    let Some(registry) = state.skill_registry.as_ref() else {
        tracing::warn!("sync_managed_allowed_skills: skipped (skill_registry is None)");
        return Ok(());
    };

    tracing::info!(
        registry_url = %catalog.registry_url(),
        "sync_managed_allowed_skills: syncing managed skill list"
    );

    // Refresh remote catalog cache first so subsequent slug lookups hit fresh data.
    sync_catalog_skill_list(catalog.as_ref()).await;

    let allowed = load_managed_allowed_skill_names(state).await?;
    if allowed.is_empty() {
        tracing::warn!("sync_managed_allowed_skills: allowed skill set is empty — no signed policy or fallback data");
        return Ok(());
    }
    tracing::info!(allowed_count = allowed.len(), allowed = ?allowed, "sync_managed_allowed_skills: allowed skill names loaded");

    let installed = installed_skill_names(registry)?;
    let missing = missing_allowed_skill_names(&allowed, &installed);
    let installed_now = install_missing_allowed_skills(catalog.as_ref(), registry, &missing).await;
    tracing::info!(
        installed_count = installed.len(),
        missing_count = missing.len(),
        missing = ?missing,
        installed_now_count = installed_now.len(),
        installed_now = ?installed_now,
        "sync_managed_allowed_skills: managed skills synchronized"
    );
    Ok(())
}

async fn sync_catalog_skill_list(catalog: &ironclaw::skills::catalog::SkillCatalog) {
    let outcome = catalog.search("").await;
    if let Some(error) = outcome.error {
        tracing::warn!(error = %error, "sync_catalog_skill_list: registry list sync failed");
        return;
    }

    tracing::info!(
        result_count = outcome.results.len(),
        "sync_catalog_skill_list: registry list synced"
    );
}

async fn load_managed_allowed_skill_names(state: &AppState) -> Result<HashSet<String>, String> {
    let Some(db) = state.db.as_ref() else {
        tracing::warn!(
            context = "load_managed_allowed_skill_names",
            "db is None, cannot load policy"
        );
        return Ok(HashSet::new());
    };

    if let Some(skills) =
        load_signed_policy_skill_names(db, &state.scope_id, "load_managed_allowed_skill_names")
            .await?
    {
        return Ok(skills);
    }

    tracing::debug!(
        context = "load_managed_allowed_skill_names",
        "falling back to setting key"
    );
    let skills = load_string_set(state, MANAGED_ALLOWED_SKILLS_SETTING_KEY).await?;
    log_loaded_skill_allowlist(
        "load_managed_allowed_skill_names",
        "managed_setting",
        &skills,
    );
    Ok(skills)
}

fn installed_skill_names(
    registry: &Arc<std::sync::RwLock<ironclaw::skills::SkillRegistry>>,
) -> Result<HashSet<String>, String> {
    let guard = registry
        .read()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    Ok(guard
        .skills()
        .iter()
        .map(|skill| skill.manifest.name.clone())
        .collect())
}

fn missing_allowed_skill_names(
    allowed: &HashSet<String>,
    installed: &HashSet<String>,
) -> Vec<String> {
    let mut missing: Vec<String> = allowed
        .iter()
        .filter(|name| !installed.contains(*name))
        .cloned()
        .collect();
    missing.sort();
    missing
}

async fn install_missing_allowed_skills(
    catalog: &ironclaw::skills::catalog::SkillCatalog,
    registry: &Arc<std::sync::RwLock<ironclaw::skills::SkillRegistry>>,
    missing_names: &[String],
) -> Vec<String> {
    let mut installed_names = Vec::new();

    for name in missing_names {
        let Some(slug) = find_catalog_slug_for_name(catalog, name).await else {
            tracing::warn!(
                skill = %name,
                "install_missing_allowed_skills: skill not found in registry"
            );
            continue;
        };

        match install_skill_from_catalog_slug(catalog, registry, &slug).await {
            Ok(true) => installed_names.push(name.clone()),
            Ok(false) => {
                tracing::info!(
                    skill = %name,
                    "install_missing_allowed_skills: skill already installed"
                );
            }
            Err(error) => {
                tracing::warn!(
                    skill = %name,
                    error = %error,
                    "install_missing_allowed_skills: install failed"
                );
            }
        }
    }

    installed_names
}

async fn find_catalog_slug_for_name(
    catalog: &ironclaw::skills::catalog::SkillCatalog,
    expected_name: &str,
) -> Option<String> {
    let outcome = catalog.search(expected_name).await;
    let expected = expected_name.trim().to_lowercase();

    if let Some(ref error) = outcome.error {
        tracing::warn!(
            skill = %expected_name,
            error = %error,
            "find_catalog_slug_for_name: catalog search returned error"
        );
    }

    tracing::debug!(
        skill = %expected_name,
        result_count = outcome.results.len(),
        result_names = ?outcome.results.iter().map(|e| &e.name).collect::<Vec<_>>(),
        "find_catalog_slug_for_name: search results"
    );

    outcome
        .results
        .iter()
        .find(|entry| entry.name.trim().to_lowercase() == expected)
        .map(|entry| {
            tracing::info!(skill = %expected_name, slug = %entry.slug, "find_catalog_slug_for_name: matched");
            entry.slug.clone()
        })
}

async fn install_skill_from_catalog_slug(
    catalog: &ironclaw::skills::catalog::SkillCatalog,
    registry: &Arc<std::sync::RwLock<ironclaw::skills::SkillRegistry>>,
    slug: &str,
) -> Result<bool, String> {
    let download_url = ironclaw::skills::catalog::skill_download_url(catalog.registry_url(), slug);
    tracing::info!(
        slug = %slug,
        download_url = %download_url,
        registry_url = %catalog.registry_url(),
        "install_skill_from_catalog_slug: downloading skill"
    );

    let content = fetch_catalog_skill_markdown(&download_url).await?;
    tracing::info!(
        slug = %slug,
        content_len = content.len(),
        content_preview = %content.chars().take(120).collect::<String>(),
        "install_skill_from_catalog_slug: downloaded content"
    );

    if content.is_empty() {
        return Err(format!(
            "Downloaded SKILL.md is empty for slug={slug} from {download_url}"
        ));
    }

    // Extract skill name from YAML frontmatter to use as directory name.
    // This avoids the _pending overwrite bug where all managed skills shared
    // a single _pending directory and overwrote each other.
    let target_dir_name = extract_skill_name_from_content(&content)
        .unwrap_or_else(|| format!("_managed_{}", slug.chars().take(8).collect::<String>()));

    let install_dir = {
        let guard = registry
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard.install_target_dir().to_path_buf()
    };

    let (name, loaded) = ironclaw::skills::SkillRegistry::prepare_install_to_disk(
        &install_dir,
        &target_dir_name,
        &content,
    )
    .await
    .map_err(|e| format!("Failed to install managed skill from registry: {}", e))?;

    tracing::info!(
        slug = %slug,
        resolved_name = %name,
        install_dir = %install_dir.display(),
        "install_skill_from_catalog_slug: prepared to disk, committing"
    );

    let mut guard = registry
        .write()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    if guard.has(&name) {
        tracing::info!(name = %name, "install_skill_from_catalog_slug: already installed, skipping");
        return Ok(false);
    }

    guard
        .commit_install(&name, loaded)
        .map_err(|e| format!("Failed to commit managed skill install: {}", e))?;
    tracing::info!(name = %name, "install_skill_from_catalog_slug: committed successfully");
    Ok(true)
}

async fn fetch_catalog_skill_markdown(download_url: &str) -> Result<String, String> {
    tracing::debug!(url = %download_url, "fetch_catalog_skill_markdown: sending GET request");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download skill package: {}", e))?;

    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(none)")
        .to_string();
    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(none)")
        .to_string();

    tracing::info!(
        url = %download_url,
        status = %status,
        content_type = %content_type,
        content_length = %content_length,
        "fetch_catalog_skill_markdown: received response"
    );

    if !status.is_success() {
        return Err(format!("Skill download returned status {}", status));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read skill package content: {}", e))?;

    tracing::info!(
        url = %download_url,
        text_len = text.len(),
        "fetch_catalog_skill_markdown: body read complete"
    );

    Ok(text)
}

/// Extract the `name` field from a SKILL.md YAML frontmatter.
///
/// Avoids pulling in a full YAML parser for a single field.
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

async fn install_managed_skill_on_demand(state: &AppState, name: &str) -> Result<(), String> {
    if !managed_mode_enabled() {
        return Ok(());
    }

    let catalog = state
        .skill_catalog
        .as_ref()
        .ok_or("Skill catalog not available")?;
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    let slug = find_catalog_slug_for_name(catalog.as_ref(), name)
        .await
        .ok_or_else(|| format!("Managed skill '{}' not found in registry list", name))?;

    install_skill_from_catalog_slug(catalog.as_ref(), registry, &slug)
        .await
        .map(|_| ())
        .map_err(|error| format!("Failed to install managed skill '{}': {}", name, error))
}

#[tauri::command]
pub async fn ic_enable_skill(state: State<'_, EngineState>, name: String) -> Result<(), String> {
    enable_skill_from_state(state.get()?, &name).await
}

pub(crate) async fn enable_skill_from_state(state: &AppState, name: &str) -> Result<(), String> {
    ensure_skill_allowed_by_policy(state, name).await?;
    if !has_skill(state, name)? {
        install_managed_skill_on_demand(state, name).await?;
    }
    ensure_skill_exists(state, name)?;
    set_skill_enabled_with_persist(state, name, true).await?;
    tracing::info!(skill = %name, "Skill enabled");
    Ok(())
}

#[tauri::command]
pub async fn ic_disable_skill(state: State<'_, EngineState>, name: String) -> Result<(), String> {
    disable_skill_from_state(state.get()?, &name).await
}

pub(crate) async fn disable_skill_from_state(state: &AppState, name: &str) -> Result<(), String> {
    ensure_skill_exists(state, name)?;
    set_skill_enabled_with_persist(state, name, false).await?;
    tracing::info!(skill = %name, "Skill disabled");
    Ok(())
}

/// 搜索技能目录。
#[tauri::command]
pub async fn ic_search_skills(
    state: State<'_, EngineState>,
    query: String,
) -> Result<Vec<CatalogSearchResult>, String> {
    let managed_mode = managed_mode_enabled();
    let registry_url = configured_registry_url();
    validate_skill_catalog_registry_policy(managed_mode, registry_url.as_deref())?;

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
    install_skill_content_from_state(state.get()?, &content).await
}

pub(crate) async fn install_skill_content_from_state(
    state: &AppState,
    content: &str,
) -> Result<String, String> {
    validate_skill_install_source(managed_mode_enabled())?;

    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    // 获取安装目标目录（短锁，立即释放）
    let install_dir = {
        let guard = registry
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard.install_target_dir().to_path_buf()
    };

    // Phase 1: 写入文件系统（无锁，可 await）
    let (name, loaded) =
        ironclaw::skills::SkillRegistry::prepare_install_to_disk(&install_dir, "_pending", content)
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
    uninstall_skill_from_state(state.get()?, &name).await
}

pub(crate) async fn uninstall_skill_from_state(state: &AppState, name: &str) -> Result<(), String> {
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;

    // Phase 1: 验证并获取路径（短锁，同步）
    let path = {
        let guard = registry
            .read()
            .map_err(|e| format!("Lock poisoned: {}", e))?;
        guard
            .validate_remove(name)
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
            .commit_remove(name)
            .map_err(|e| format!("Failed to commit removal: {}", e))?;
    }

    tracing::info!(skill = %name, "Skill uninstalled");
    Ok(())
}

fn ensure_skill_exists(state: &AppState, name: &str) -> Result<(), String> {
    if has_skill(state, name)? {
        return Ok(());
    }
    Err(format!("Skill not found: {}", name))
}

fn has_skill(state: &AppState, name: &str) -> Result<bool, String> {
    let registry = state.skill_registry.as_ref().ok_or("Skills not enabled")?;
    let guard = registry
        .read()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    Ok(guard.has(name))
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
        return Err(format!("{}; rollback failed: {}", error, rollback_error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        disable_skill_from_state, enable_skill_from_state, extract_skill_name_from_content,
        filter_skill_infos_by_allowlist, install_missing_allowed_skills,
        install_skill_content_from_state, list_skills_from_state, loaded_skill_to_info,
        missing_allowed_skill_names, skill_allowed_by_policy, uninstall_skill_from_state,
        validate_skill_catalog_registry_policy, validate_skill_install_source, SkillInfo,
        DISABLED_SKILLS_SETTING_KEY,
    };
    use crate::state::AppState;
    use dasclaw_runtime::context::ContextManager;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};
    use tokio::sync::mpsc;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn loaded_skill(
        name: &str,
        source: ironclaw::skills::SkillSource,
        trust: ironclaw::skills::SkillTrust,
        keywords: Vec<&str>,
    ) -> ironclaw::skills::LoadedSkill {
        let activation = ironclaw::skills::ActivationCriteria {
            keywords: keywords.into_iter().map(str::to_string).collect(),
            ..Default::default()
        };
        ironclaw::skills::LoadedSkill {
            manifest: ironclaw::skills::SkillManifest {
                name: name.to_string(),
                version: "1.2.3".to_string(),
                description: "Mapped skill".to_string(),
                activation,
                metadata: None,
            },
            prompt_content: "Prompt".to_string(),
            trust,
            source,
            content_hash: "hash".to_string(),
            compiled_patterns: Vec::new(),
            lowercased_keywords: Vec::new(),
            lowercased_exclude_keywords: Vec::new(),
            lowercased_tags: Vec::new(),
        }
    }

    fn sample_skill(name: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            source: "user".to_string(),
            trust: "installed".to_string(),
            keywords: Vec::new(),
            enabled: true,
        }
    }

    fn managed_skill_content(name: &str) -> String {
        format!("---\nname: {name}\ndescription: Installed skill\n---\n\nPrompt.\n")
    }

    fn local_skill_content(name: &str) -> String {
        format!(
            "---\nname: {name}\nversion: 1.0.0\ndescription: Local round trip skill\nactivation:\n  keywords:\n    - roundtrip\n---\n\nUse this skill for round trip tests.\n"
        )
    }

    async fn test_db(tempdir: &tempfile::TempDir) -> Arc<dyn ironclaw::db::Database> {
        let db_path = tempdir.path().join("skills-state.db");
        let backend = ironclaw::db::libsql::LibSqlBackend::new_local(&db_path)
            .await
            .expect("file-backed libsql backend should initialize");
        <ironclaw::db::libsql::LibSqlBackend as ironclaw::db::Database>::run_migrations(&backend)
            .await
            .expect("migrations should run");
        Arc::new(backend)
    }

    fn test_registry(tempdir: &tempfile::TempDir) -> Arc<RwLock<ironclaw::skills::SkillRegistry>> {
        Arc::new(RwLock::new(
            ironclaw::skills::SkillRegistry::new(tempdir.path().join("user-skills"))
                .with_installed_dir(tempdir.path().join("installed-skills")),
        ))
    }

    async fn test_app_state_with_skills() -> (AppState, tempfile::TempDir) {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let db = test_db(&tempdir).await;
        let registry = test_registry(&tempdir);
        let (tx, _rx) = mpsc::channel(1);

        let safety_config = ironclaw::safety::SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        };
        let safety = Arc::new(ironclaw::safety::SafetyLayer::new(&safety_config));
        let safety_bridge = Arc::new(crate::safety_bridge::SafetyBridge::new(
            Arc::clone(&safety),
            None,
            None,
        ));
        let egress: Arc<dyn dasclaw_core::EgressGate> = Arc::new(
            dasclaw_safety::egress_gate::IronclawEgressGate::new(Arc::clone(&safety)),
        );
        let attachment_scanner = Arc::new(
            crate::safety_attachment_scanner::AttachmentScanner::new(Arc::clone(&egress)),
        );
        let model_override = Arc::new(std::sync::RwLock::new(None));
        let stub_llm: Arc<dyn ironclaw::llm::LlmProvider> = Arc::new(StubLlmProvider);
        let model_switch = Arc::new(crate::model_switch::ModelSwitchProvider::new(
            Arc::clone(&stub_llm),
            Arc::clone(&model_override),
        ));

        let state = AppState {
            msg_sender: tx,
            db: Some(db),
            workspace: None,
            tools: Arc::new(ironclaw::tools::ToolRegistry::new()),
            extension_manager: None,
            skill_registry: Some(registry),
            skill_catalog: None,
            skills_config: ironclaw::config::SkillsConfig::default(),
            safety,
            safety_bridge,
            attachment_scanner,
            egress,
            context_manager: Arc::new(ContextManager::new(5)),
            conversation_tracker: Arc::new(crate::conversation_tracker::ConversationTracker::new(
                "skills-ipc-owner".to_string(),
            )),
            data_reporter: Arc::new(crate::data_reporter::DataReporter::new_for_test()),
            scope_id: "skills-ipc-owner".to_string(),
            backend_user_id: Arc::new(std::sync::RwLock::new(None)),
            llm: Arc::clone(&model_switch) as _,
            model_override,
            model_switch,
            provider_base_url: std::sync::RwLock::new(String::new()),
            initial_provider: Arc::clone(&stub_llm),
            initial_base_url: String::new(),
            log_broadcaster: Arc::new(ironclaw::channels::web::log_layer::LogBroadcaster::new()),
            log_clear_offset: std::sync::atomic::AtomicUsize::new(0),
            routine_engine_slot: Arc::new(tokio::sync::RwLock::new(None)),
            scheduler_slot: Arc::new(tokio::sync::RwLock::new(None)),
            disabled_skills: std::sync::RwLock::new(std::collections::HashSet::new()),
            disabled_extensions: std::sync::RwLock::new(std::collections::HashSet::new()),
        };

        (state, tempdir)
    }

    struct StubLlmProvider;

    #[async_trait::async_trait]
    impl ironclaw::llm::LlmProvider for StubLlmProvider {
        fn model_name(&self) -> &str {
            "stub-model"
        }

        fn cost_per_token(&self) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
            (rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)
        }

        async fn complete(
            &self,
            _req: ironclaw::llm::CompletionRequest,
        ) -> Result<ironclaw::llm::CompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }

        async fn complete_with_tools(
            &self,
            _req: ironclaw::llm::ToolCompletionRequest,
        ) -> Result<ironclaw::llm::ToolCompletionResponse, ironclaw::error::LlmError> {
            Err(ironclaw::error::LlmError::RequestFailed {
                provider: "stub".into(),
                reason: "not implemented".into(),
            })
        }
    }

    #[test]
    fn test_validate_skill_install_source_managed_mode_rejects_content_install() {
        let result = validate_skill_install_source(true);
        assert!(
            result.is_err(),
            "managed mode should reject local SKILL.md content install"
        );
        let message = result.err().unwrap_or_default();
        assert!(
            message.contains("Managed mode"),
            "error should mention managed mode, actual: {}",
            message
        );
    }

    #[test]
    fn test_validate_skill_install_source_non_managed_mode_allows_content_install() {
        let result = validate_skill_install_source(false);
        assert!(
            result.is_ok(),
            "non-managed mode should allow content install"
        );
    }

    #[test]
    fn test_validate_skill_catalog_registry_policy_managed_requires_registry_url() {
        let result = validate_skill_catalog_registry_policy(true, None);
        assert!(
            result.is_err(),
            "managed mode should require CLAWHUB_REGISTRY"
        );
    }

    #[test]
    fn test_validate_skill_catalog_registry_policy_managed_accepts_registry_url() {
        let result =
            validate_skill_catalog_registry_policy(true, Some("https://admin.example.com/api/v1"));
        assert!(
            result.is_ok(),
            "managed mode should accept configured admin registry"
        );
    }

    #[test]
    fn test_validate_skill_catalog_registry_policy_non_managed_accepts_missing_registry() {
        let result = validate_skill_catalog_registry_policy(false, None);
        assert!(
            result.is_ok(),
            "non-managed mode should allow missing CLAWHUB_REGISTRY"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn req_skills_install_enable_disable_uninstall_state_round_trip() {
        std::env::remove_var("MANAGED_MODE");
        std::env::remove_var("CLAWHUB_REGISTRY");

        let (state, _tempdir) = test_app_state_with_skills().await;
        let skill_name = "roundtrip-skill";

        let installed = install_skill_content_from_state(&state, &local_skill_content(skill_name))
            .await
            .expect("local SKILL.md install should update the real registry");
        assert_eq!(installed, skill_name);

        let listed = list_skills_from_state(&state)
            .await
            .expect("list should read installed registry state");
        let skill = listed
            .iter()
            .find(|skill| skill.name == skill_name)
            .expect("installed skill should be listed");
        assert_eq!(skill.source, "user");
        assert_eq!(skill.trust, "installed");
        assert!(skill.enabled, "newly installed skills should be enabled");

        disable_skill_from_state(&state, skill_name)
            .await
            .expect("disable should persist state");
        let listed = list_skills_from_state(&state)
            .await
            .expect("list should reflect disabled state");
        let skill = listed
            .iter()
            .find(|skill| skill.name == skill_name)
            .expect("disabled skill should still be listed");
        assert!(!skill.enabled, "disabled skill should be reflected in DTO");

        let disabled = state
            .db
            .as_ref()
            .expect("db should exist")
            .get_setting(&state.scope_id, DISABLED_SKILLS_SETTING_KEY)
            .await
            .expect("disabled skill setting should be readable")
            .expect("disabled skill setting should be persisted");
        assert_eq!(disabled, serde_json::json!([skill_name]));

        enable_skill_from_state(&state, skill_name)
            .await
            .expect("enable should persist state");
        let listed = list_skills_from_state(&state)
            .await
            .expect("list should reflect enabled state");
        let skill = listed
            .iter()
            .find(|skill| skill.name == skill_name)
            .expect("enabled skill should still be listed");
        assert!(skill.enabled, "enabled skill should be reflected in DTO");

        let disabled = state
            .db
            .as_ref()
            .expect("db should exist")
            .get_setting(&state.scope_id, DISABLED_SKILLS_SETTING_KEY)
            .await
            .expect("disabled skill setting should be readable")
            .expect("disabled skill setting should remain persisted");
        assert_eq!(disabled, serde_json::json!([]));

        uninstall_skill_from_state(&state, skill_name)
            .await
            .expect("uninstall should update registry and disk state");
        let listed = list_skills_from_state(&state)
            .await
            .expect("list should read post-uninstall registry state");
        assert!(
            listed.iter().all(|skill| skill.name != skill_name),
            "uninstalled skill should no longer be listed"
        );
    }

    #[test]
    fn test_missing_allowed_skill_names_returns_sorted_missing_names() {
        let allowed = HashSet::from([
            "code-review-expert".to_string(),
            "code-simplifier".to_string(),
            "other-skill".to_string(),
        ]);
        let installed = HashSet::from(["other-skill".to_string()]);

        let missing = missing_allowed_skill_names(&allowed, &installed);
        assert_eq!(
            missing,
            vec![
                "code-review-expert".to_string(),
                "code-simplifier".to_string()
            ]
        );
    }

    #[test]
    fn test_missing_allowed_skill_names_returns_empty_when_all_installed() {
        let allowed = HashSet::from(["code-review-expert".to_string()]);
        let installed = HashSet::from(["code-review-expert".to_string()]);

        let missing = missing_allowed_skill_names(&allowed, &installed);
        assert!(missing.is_empty(), "no missing skills expected");
    }

    #[test]
    fn test_filter_skill_infos_by_allowlist_hides_unauthorized_skills() {
        let skills = vec![sample_skill("code-simplifier"), sample_skill("other-skill")];
        let allowed = HashSet::from(["code-simplifier".to_string()]);

        let filtered = filter_skill_infos_by_allowlist(skills, Some(&allowed));

        assert_eq!(filtered.len(), 1, "only allowed skills should remain");
        assert_eq!(filtered[0].name, "code-simplifier");
    }

    #[test]
    fn test_skill_allowed_by_policy_requires_membership_when_allowlist_present() {
        let allowed = HashSet::from(["code-simplifier".to_string()]);

        assert!(skill_allowed_by_policy("code-simplifier", Some(&allowed)));
        assert!(!skill_allowed_by_policy("other-skill", Some(&allowed)));
        assert!(skill_allowed_by_policy("other-skill", None));
    }

    #[test]
    fn req_skills_list_maps_loaded_skill_to_frontend_info() {
        let skill = loaded_skill(
            "code-review-expert",
            ironclaw::skills::SkillSource::User(PathBuf::from("/tmp/skills/code-review-expert")),
            ironclaw::skills::SkillTrust::Installed,
            vec!["review", "audit"],
        );

        let info = loaded_skill_to_info(&skill, false);

        assert_eq!(info.name, "code-review-expert");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.description, "Mapped skill");
        assert_eq!(info.source, "user");
        assert_eq!(info.trust, "installed");
        assert_eq!(info.keywords, vec!["review", "audit"]);
        assert!(!info.enabled);
    }

    #[test]
    fn req_skills_list_caps_keywords_from_loaded_skill() {
        let skill = loaded_skill(
            "verbose-skill",
            ironclaw::skills::SkillSource::Workspace(PathBuf::from("/tmp/workspace/skills")),
            ironclaw::skills::SkillTrust::Trusted,
            vec!["one", "two", "three", "four", "five", "six"],
        );

        let info = loaded_skill_to_info(&skill, true);

        assert_eq!(info.source, "workspace");
        assert_eq!(info.trust, "trusted");
        assert_eq!(info.keywords, vec!["one", "two", "three", "four", "five"]);
        assert!(info.enabled);
    }

    #[test]
    fn test_extract_skill_name_from_content() {
        let content = "---\nname: code-review-expert\nversion: 0.1.0\n---\nBody text";
        assert_eq!(
            extract_skill_name_from_content(content),
            Some("code-review-expert".to_string())
        );
    }

    #[test]
    fn test_extract_skill_name_quoted() {
        let content = "---\nname: \"my-skill\"\nversion: 0.1.0\n---\n";
        assert_eq!(
            extract_skill_name_from_content(content),
            Some("my-skill".to_string())
        );
    }

    #[test]
    fn test_extract_skill_name_missing() {
        let content = "---\nversion: 0.1.0\n---\nNo name field";
        assert_eq!(extract_skill_name_from_content(content), None);
    }

    #[test]
    fn test_extract_skill_name_no_frontmatter() {
        let content = "Just some text, no YAML frontmatter";
        assert_eq!(extract_skill_name_from_content(content), None);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_install_missing_allowed_skills_installs_catalog_entries() {
        let server = MockServer::start().await;
        let skill_name = "code-review-expert";
        let slug = "owner/code-review-expert";

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .and(query_param("q", skill_name))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "slug": slug,
                    "displayName": skill_name,
                    "summary": "Installed skill",
                    "version": "1.0.0",
                    "score": 1.0
                }]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v1/download"))
            .and(query_param("slug", slug))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(managed_skill_content(skill_name)),
            )
            .mount(&server)
            .await;

        std::env::set_var("CLAWHUB_REGISTRY", server.uri());
        let catalog = ironclaw::skills::catalog::SkillCatalog::new();
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let registry = Arc::new(RwLock::new(
            ironclaw::skills::SkillRegistry::new(tempdir.path().join("user-skills"))
                .with_installed_dir(tempdir.path().join("installed-skills")),
        ));

        let installed =
            install_missing_allowed_skills(&catalog, &registry, &[skill_name.to_string()]).await;

        std::env::remove_var("CLAWHUB_REGISTRY");

        assert_eq!(installed, vec![skill_name.to_string()]);
        let guard = registry.read().expect("read registry");
        assert!(guard.has(skill_name));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_install_missing_allowed_skills_continues_after_lookup_failure() {
        let server = MockServer::start().await;
        let good_name = "code-review-expert";
        let good_slug = "owner/code-review-expert";
        let missing_name = "missing-skill";

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .and(query_param("q", good_name))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "slug": good_slug,
                    "displayName": good_name,
                    "summary": "Installed skill",
                    "version": "1.0.0",
                    "score": 1.0
                }]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v1/search"))
            .and(query_param("q", missing_name))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": []
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/v1/download"))
            .and(query_param("slug", good_slug))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(managed_skill_content(good_name)),
            )
            .mount(&server)
            .await;

        std::env::set_var("CLAWHUB_REGISTRY", server.uri());
        let catalog = ironclaw::skills::catalog::SkillCatalog::new();
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let registry = Arc::new(RwLock::new(
            ironclaw::skills::SkillRegistry::new(tempdir.path().join("user-skills"))
                .with_installed_dir(tempdir.path().join("installed-skills")),
        ));

        let installed = install_missing_allowed_skills(
            &catalog,
            &registry,
            &[good_name.to_string(), missing_name.to_string()],
        )
        .await;

        std::env::remove_var("CLAWHUB_REGISTRY");

        assert_eq!(installed, vec![good_name.to_string()]);
        let guard = registry.read().expect("read registry");
        assert!(guard.has(good_name));
        assert!(!guard.has(missing_name));
    }
}
