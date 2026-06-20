use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dasclaw_app_server_protocol::{
    ServiceHealth, ServiceName, SkillErrorInfo, SkillMetadata, SkillScope, SkillsConfigWriteParams,
    SkillsConfigWriteResponse, SkillsListEntry, SkillsListParams, SkillsListResponse,
};

use crate::AppServerError;
use crate::app_services::SkillsService;

const MAX_SKILL_MD_BYTES: u64 = 64 * 1024;

#[derive(Clone, Default)]
pub struct AppServerSkillsService {
    overrides: Arc<Mutex<HashMap<String, bool>>>,
    known_targets: Arc<Mutex<HashMap<String, String>>>,
}

impl AppServerSkillsService {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SkillsService for AppServerSkillsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Skills)
    }

    fn list(&self, params: SkillsListParams) -> Result<SkillsListResponse, AppServerError> {
        let cwds = if params.cwds.is_empty() {
            vec![
                std::env::current_dir()
                    .map_err(|error| {
                        AppServerError::capability_unavailable(
                            "skills",
                            format!("failed to resolve current directory: {error}"),
                        )
                    })?
                    .display()
                    .to_string(),
            ]
        } else {
            params.cwds
        };
        let extra_roots = extra_roots_by_cwd(params.per_cwd_extra_user_roots);
        let overrides = self
            .overrides
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();

        let data: Vec<_> = cwds
            .into_iter()
            .map(|cwd| list_cwd_skills(&cwd, extra_roots.get(&cwd), &overrides))
            .collect();
        self.record_known_targets(&data);
        Ok(SkillsListResponse { data })
    }

    fn write_config(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        let canonical_path = self.resolve_config_target(params.path, params.name)?;
        self.overrides
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(canonical_path, params.enabled);
        Ok(SkillsConfigWriteResponse {
            effective_enabled: params.enabled,
        })
    }
}

impl AppServerSkillsService {
    fn resolve_config_target(
        &self,
        path: Option<String>,
        name: Option<String>,
    ) -> Result<String, AppServerError> {
        if let Some(path) = path {
            return resolve_skill_path(&path);
        }
        let name = name
            .ok_or_else(|| AppServerError::invalid_request("skills", "path or name is required"))?;
        self.known_targets
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(&name)
            .cloned()
            .ok_or_else(|| AppServerError::invalid_request("skills", "skill not found"))
    }

    fn record_known_targets(&self, entries: &[SkillsListEntry]) {
        let mut known = self
            .known_targets
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        known.clear();
        let skills: Vec<_> = entries.iter().flat_map(|entry| &entry.skills).collect();
        let mut name_counts = HashMap::new();
        for skill in &skills {
            *name_counts.entry(skill.name.clone()).or_insert(0usize) += 1;
        }
        for skill in skills {
            known.insert(skill.path.clone(), skill.path.clone());
            if name_counts.get(&skill.name).copied() == Some(1) {
                known.insert(skill.name.clone(), skill.path.clone());
            }
        }
    }
}

fn resolve_skill_path(path: &str) -> Result<String, AppServerError> {
    let path = PathBuf::from(path);
    let content = read_skill(&path).map_err(|message| {
        AppServerError::invalid_request("skills", format!("invalid skill path: {message}"))
    })?;
    parse_skill_md(&path, &content)
        .map(|skill| skill.path)
        .map_err(|message| {
            AppServerError::invalid_request("skills", format!("invalid skill path: {message}"))
        })
}

fn extra_roots_by_cwd(
    roots: Option<Vec<dasclaw_app_server_protocol::SkillsListExtraRootsForCwd>>,
) -> HashMap<String, Vec<String>> {
    roots
        .unwrap_or_default()
        .into_iter()
        .map(|entry| (entry.cwd, entry.extra_user_roots))
        .collect()
}

fn list_cwd_skills(
    cwd: &str,
    extra_roots: Option<&Vec<String>>,
    overrides: &HashMap<String, bool>,
) -> SkillsListEntry {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    for root in skill_roots(cwd, extra_roots) {
        discover_root(&root, overrides, &mut skills, &mut errors);
    }
    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.path.cmp(&right.path))
    });
    SkillsListEntry {
        cwd: cwd.to_string(),
        skills,
        errors,
    }
}

fn skill_roots(cwd: &str, extra_roots: Option<&Vec<String>>) -> Vec<PathBuf> {
    let mut roots = vec![Path::new(cwd).join(".codex").join("skills")];
    if let Some(extra_roots) = extra_roots {
        roots.extend(extra_roots.iter().map(PathBuf::from));
    }
    roots
}

fn discover_root(
    root: &Path,
    overrides: &HashMap<String, bool>,
    skills: &mut Vec<SkillMetadata>,
    errors: &mut Vec<SkillErrorInfo>,
) {
    if !root.exists() {
        return;
    }
    if root.join("SKILL.md").is_file() {
        load_skill(root.join("SKILL.md"), overrides, skills, errors);
    }
    let Ok(entries) = fs::read_dir(root) else {
        errors.push(SkillErrorInfo {
            path: root.display().to_string(),
            message: "failed to read skills root".to_string(),
        });
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path().join("SKILL.md");
        if path.is_file() {
            load_skill(path, overrides, skills, errors);
        }
    }
}

fn load_skill(
    path: PathBuf,
    overrides: &HashMap<String, bool>,
    skills: &mut Vec<SkillMetadata>,
    errors: &mut Vec<SkillErrorInfo>,
) {
    match read_skill(&path).and_then(|content| parse_skill_md(&path, &content)) {
        Ok(mut skill) => {
            if let Some(enabled) = overrides
                .get(&skill.path)
                .or_else(|| overrides.get(&skill.name))
                .copied()
            {
                skill.enabled = enabled;
            }
            skills.push(skill);
        }
        Err(message) => errors.push(SkillErrorInfo {
            path: path.display().to_string(),
            message,
        }),
    }
}

fn read_skill(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect SKILL.md: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("symlinked SKILL.md is not allowed".to_string());
    }
    if metadata.len() > MAX_SKILL_MD_BYTES {
        return Err(format!(
            "SKILL.md is too large: {} bytes exceeds {}",
            metadata.len(),
            MAX_SKILL_MD_BYTES
        ));
    }
    fs::read_to_string(path).map_err(|error| format!("failed to read SKILL.md: {error}"))
}

fn parse_skill_md(path: &Path, content: &str) -> Result<SkillMetadata, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let trimmed = content.trim_start_matches(['\n', '\r']);
    if !trimmed.starts_with("---") {
        return Err("SKILL.md is missing YAML frontmatter".to_string());
    }
    let after_first = &trimmed[3..];
    let Some(first_newline) = after_first.find('\n') else {
        return Err("SKILL.md is missing YAML frontmatter".to_string());
    };
    let frontmatter = &after_first[first_newline + 1..];
    let Some((yaml, _body)) = frontmatter.split_once("\n---") else {
        return Err("SKILL.md is missing closing frontmatter delimiter".to_string());
    };

    let fields = parse_frontmatter_scalars(yaml);
    let name = fields
        .get("name")
        .cloned()
        .or_else(|| {
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .ok_or_else(|| "SKILL.md frontmatter must include name".to_string())?;
    let description = fields.get("description").cloned().unwrap_or_default();
    let short_description = fields.get("short_description").cloned();

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        interface: None,
        dependencies: None,
        path: path.display().to_string(),
        scope: SkillScope::Repo,
        enabled: true,
    })
}

fn parse_frontmatter_scalars(yaml: &str) -> HashMap<String, String> {
    yaml.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (key, value) = line.split_once(':')?;
            let key = key.trim();
            if key.is_empty() || key.starts_with('#') {
                return None;
            }
            Some((key.to_string(), unquote_yaml_scalar(value.trim())))
        })
        .collect()
}

fn unquote_yaml_scalar(value: &str) -> String {
    value.trim_matches('"').trim_matches('\'').to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use dasclaw_app_server_protocol::{
        SkillsConfigWriteParams, SkillsListExtraRootsForCwd, SkillsListParams,
    };

    use super::AppServerSkillsService;
    use crate::app_services::SkillsService;

    #[test]
    fn app_server_skills_service_lists_skill_md_from_cwd_and_extra_roots() {
        let temp = TestDir::new("app_server_skills_list");
        let cwd = temp.path().join("repo");
        write_skill(
            &cwd.join(".codex/skills/review/SKILL.md"),
            "review",
            "Review local code",
        );
        let extra_root = temp.path().join("extra-skills");
        write_skill(&extra_root.join("plan/SKILL.md"), "plan", "Write plans");
        let service = AppServerSkillsService::new();

        let response = service
            .list(SkillsListParams {
                cwds: vec![cwd.display().to_string()],
                force_reload: true,
                per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                    cwd: cwd.display().to_string(),
                    extra_user_roots: vec![extra_root.display().to_string()],
                }]),
            })
            .unwrap();

        let names: Vec<_> = response.data[0]
            .skills
            .iter()
            .map(|skill| skill.name.as_str())
            .collect();
        assert_eq!(names, vec!["plan", "review"]);
        assert_eq!(
            response.data[0].skills[0].scope,
            dasclaw_app_server_protocol::SkillScope::Repo
        );
        assert!(response.data[0].errors.is_empty());
    }

    #[test]
    fn app_server_skills_config_write_changes_enabled_state_for_same_owner() {
        let temp = TestDir::new("app_server_skills_write");
        let cwd = temp.path().join("repo");
        let skill_path = cwd.join(".codex/skills/review/SKILL.md");
        write_skill(&skill_path, "review", "Review local code");
        let service = AppServerSkillsService::new();

        let before_write = service
            .list(SkillsListParams {
                cwds: vec![cwd.display().to_string()],
                ..SkillsListParams::default()
            })
            .unwrap();
        let write = service
            .write_config(SkillsConfigWriteParams {
                path: Some(skill_path.display().to_string()),
                name: None,
                enabled: false,
            })
            .unwrap();
        let response = service
            .list(SkillsListParams {
                cwds: vec![cwd.display().to_string()],
                ..SkillsListParams::default()
            })
            .unwrap();

        assert_eq!(before_write.data[0].skills[0].name, "review");
        assert!(!write.effective_enabled);
        assert_eq!(response.data[0].skills[0].name, "review");
        assert!(!response.data[0].skills[0].enabled);
    }

    #[test]
    fn app_server_skills_config_write_rejects_unknown_skill() {
        let service = AppServerSkillsService::new();

        let result = service.write_config(SkillsConfigWriteParams {
            path: None,
            name: Some("missing".to_string()),
            enabled: false,
        });

        assert!(result.is_err());
    }

    fn write_skill(path: &std::path::Path, name: &str, description: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "---\nname: {name}\ndescription: {description}\nshort_description: Short {name}\n---\nUse {name}.\n"
            ),
        )
        .unwrap();
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dasclaw_app_server_{name}_{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
