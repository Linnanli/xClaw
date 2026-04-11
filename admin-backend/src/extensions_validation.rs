use serde::Deserialize;

use crate::error::{Error, Result};

const MAX_SKILL_FILE_BYTES: usize = 64 * 1024;
const MAX_KEYWORDS: usize = 20;
const MAX_PATTERNS: usize = 5;
const MAX_TAGS: usize = 10;

#[derive(Debug, Clone)]
pub struct SkillPackageMetadata {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActivationConfig {
    keywords: Option<Vec<String>>,
    patterns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    author: Option<String>,
    activation: Option<ActivationConfig>,
    keywords: Option<Vec<String>>,
    exclude_keywords: Option<Vec<String>>,
    patterns: Option<Vec<String>>,
    tags: Option<Vec<String>>,
}

pub fn validate_skill_package(content: &str) -> Result<()> {
    if content.len() > MAX_SKILL_FILE_BYTES {
        return Err(Error::Validation("技能包文件超过 64 KiB 限制".into()));
    }

    let yaml = extract_frontmatter_yaml(content)?;
    let frontmatter = parse_frontmatter(&yaml)?;

    validate_frontmatter(&frontmatter)
}

pub fn extract_skill_metadata(content: &str) -> Result<SkillPackageMetadata> {
    let yaml = extract_frontmatter_yaml(content)?;
    let frontmatter = parse_frontmatter(&yaml)?;
    let name = validate_name(frontmatter.name.as_deref())?;

    Ok(SkillPackageMetadata {
        name,
        version: normalize_optional(frontmatter.version),
        description: normalize_optional(frontmatter.description),
        author: normalize_optional(frontmatter.author),
    })
}

fn parse_frontmatter(yaml: &str) -> Result<SkillFrontmatter> {
    serde_yaml::from_str(yaml)
        .map_err(|e| Error::Validation(format!("frontmatter YAML 解析失败: {}", e)))
}

fn extract_frontmatter_yaml(content: &str) -> Result<String> {
    if !content.starts_with("---") {
        return Err(Error::Validation(
            "技能包格式错误：缺少 YAML frontmatter（以 --- 开头）".into(),
        ));
    }

    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return Err(Error::Validation(
            "技能包格式错误：frontmatter 起始分隔符格式不正确".into(),
        ));
    }

    let mut yaml = String::new();
    for line in lines {
        if line == "---" {
            return Ok(yaml);
        }
        yaml.push_str(line);
        yaml.push('\n');
    }

    Err(Error::Validation(
        "技能包格式错误：缺少 frontmatter 结束分隔符 ---".into(),
    ))
}

fn validate_frontmatter(frontmatter: &SkillFrontmatter) -> Result<()> {
    validate_name(frontmatter.name.as_deref())?;
    validate_required_text_field(frontmatter.version.as_deref(), "version")?;
    validate_required_text_field(frontmatter.description.as_deref(), "description")?;
    validate_activation(frontmatter.activation.as_ref())?;
    validate_keyword_list(&frontmatter.keywords, "keywords")?;
    validate_keyword_list(&frontmatter.exclude_keywords, "exclude_keywords")?;
    validate_max_count(&frontmatter.patterns, MAX_PATTERNS, "patterns")?;
    validate_max_count(&frontmatter.tags, MAX_TAGS, "tags")?;
    Ok(())
}

fn validate_name(name: Option<&str>) -> Result<String> {
    let normalized = name
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::Validation("frontmatter 缺少必填字段 name".into()))?;

    if normalized.len() > 64 || !is_valid_skill_name(normalized) {
        return Err(Error::Validation(
            "name 格式无效：必须字母数字开头，仅允许 . _ -，且最长 64 字符".into(),
        ));
    }

    Ok(normalized.to_string())
}

fn validate_required_text_field(value: Option<&str>, field_name: &str) -> Result<()> {
    let normalized = value.map(str::trim).unwrap_or("");
    if normalized.is_empty() {
        return Err(Error::Validation(format!(
            "frontmatter 缺少必填字段 {}",
            field_name
        )));
    }
    Ok(())
}

fn validate_activation(activation: Option<&ActivationConfig>) -> Result<()> {
    let activation = activation
        .ok_or_else(|| Error::Validation("frontmatter 缺少必填字段 activation".into()))?;
    let has_keywords = activation
        .keywords
        .as_ref()
        .is_some_and(|items| !items.is_empty());
    let has_patterns = activation
        .patterns
        .as_ref()
        .is_some_and(|items| !items.is_empty());

    if !has_keywords && !has_patterns {
        return Err(Error::Validation(
            "activation.keywords 或 activation.patterns 至少一个非空".into(),
        ));
    }

    Ok(())
}

fn validate_keyword_list(list: &Option<Vec<String>>, field_name: &str) -> Result<()> {
    if let Some(items) = list {
        if items.len() > MAX_KEYWORDS {
            return Err(Error::Validation(format!(
                "{} 最多允许 {} 个",
                field_name, MAX_KEYWORDS
            )));
        }
        if items.iter().any(|item| item.trim().len() < 3) {
            return Err(Error::Validation(format!(
                "{} 中每个关键词至少 3 个字符",
                field_name
            )));
        }
    }

    Ok(())
}

fn validate_max_count(
    values: &Option<Vec<String>>,
    max_allowed: usize,
    field_name: &str,
) -> Result<()> {
    if values
        .as_ref()
        .is_some_and(|items| items.len() > max_allowed)
    {
        return Err(Error::Validation(format!(
            "{} 最多允许 {} 个",
            field_name, max_allowed
        )));
    }

    Ok(())
}

fn is_valid_skill_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };

    if !first.is_ascii_alphanumeric() {
        return false;
    }

    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}