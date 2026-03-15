use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub keywords: Vec<String>,
    pub trust_level: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkill {
    pub metadata: Skill,
    pub enabled: bool,
}

pub struct SkillManager {
    installed_skills: HashMap<String, InstalledSkill>,
    available_skills: Vec<Skill>,
}

impl SkillManager {
    pub fn new() -> Self {
        Self {
            installed_skills: HashMap::new(),
            available_skills: Self::default_available_skills(),
        }
    }

    fn default_available_skills() -> Vec<Skill> {
        vec![
            Skill {
                id: "code-review".to_string(),
                name: "代码审查".to_string(),
                version: "1.0.0".to_string(),
                description: "自动审查代码质量和安全性".to_string(),
                author: "IronClaw".to_string(),
                keywords: vec!["code".to_string(), "review".to_string(), "quality".to_string()],
                trust_level: "high".to_string(),
                source: "official".to_string(),
            },
            Skill {
                id: "documentation".to_string(),
                name: "文档生成".to_string(),
                version: "1.0.0".to_string(),
                description: "自动生成项目文档".to_string(),
                author: "IronClaw".to_string(),
                keywords: vec!["doc".to_string(), "generate".to_string()],
                trust_level: "high".to_string(),
                source: "official".to_string(),
            },
            Skill {
                id: "testing".to_string(),
                name: "测试生成".to_string(),
                version: "1.0.0".to_string(),
                description: "自动生成单元测试".to_string(),
                author: "IronClaw".to_string(),
                keywords: vec!["test".to_string(), "unit".to_string()],
                trust_level: "high".to_string(),
                source: "official".to_string(),
            },
        ]
    }

    pub fn get_available_skills(&self) -> Result<Vec<Skill>> {
        Ok(self.available_skills.clone())
    }

    pub fn get_installed_skills(&self) -> Result<Vec<InstalledSkill>> {
        Ok(self.installed_skills.values().cloned().collect())
    }

    pub fn install_skill(&mut self, skill_id: String) -> Result<()> {
        let skill = self
            .available_skills
            .iter()
            .find(|s| s.id == skill_id)
            .ok_or(Error::StorageError(format!("Skill {} not found", skill_id)))?
            .clone();

        self.installed_skills.insert(
            skill_id,
            InstalledSkill {
                metadata: skill,
                enabled: true,
            },
        );

        Ok(())
    }

    pub fn uninstall_skill(&mut self, skill_id: String) -> Result<()> {
        self.installed_skills
            .remove(&skill_id)
            .ok_or(Error::StorageError(format!("Skill {} not installed", skill_id)))?;

        Ok(())
    }

    pub fn enable_skill(&mut self, skill_id: String) -> Result<()> {
        let skill = self
            .installed_skills
            .get_mut(&skill_id)
            .ok_or(Error::StorageError(format!("Skill {} not installed", skill_id)))?;

        skill.enabled = true;
        Ok(())
    }

    pub fn disable_skill(&mut self, skill_id: String) -> Result<()> {
        let skill = self
            .installed_skills
            .get_mut(&skill_id)
            .ok_or(Error::StorageError(format!("Skill {} not installed", skill_id)))?;

        skill.enabled = false;
        Ok(())
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_manager_creation() {
        let manager = SkillManager::new();
        assert_eq!(manager.available_skills.len(), 3);
    }

    #[test]
    fn test_install_skill() {
        let mut manager = SkillManager::new();
        assert!(manager.install_skill("code-review".to_string()).is_ok());
        assert_eq!(manager.installed_skills.len(), 1);
    }

    #[test]
    fn test_uninstall_skill() {
        let mut manager = SkillManager::new();
        manager.install_skill("code-review".to_string()).unwrap();
        assert!(manager.uninstall_skill("code-review".to_string()).is_ok());
        assert_eq!(manager.installed_skills.len(), 0);
    }

    #[test]
    fn test_enable_disable_skill() {
        let mut manager = SkillManager::new();
        manager.install_skill("code-review".to_string()).unwrap();
        
        manager.disable_skill("code-review".to_string()).unwrap();
        let skill = manager.installed_skills.get("code-review").unwrap();
        assert!(!skill.enabled);

        manager.enable_skill("code-review".to_string()).unwrap();
        let skill = manager.installed_skills.get("code-review").unwrap();
        assert!(skill.enabled);
    }
}
