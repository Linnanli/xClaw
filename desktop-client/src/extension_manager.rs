use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub min_memory_mb: Option<u32>,
    pub min_disk_mb: Option<u32>,
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tools: Vec<String>,
    pub permissions: Vec<String>,
    pub resource_requirements: Option<ResourceRequirements>,
    pub auto_update: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub metadata: ExtensionMetadata,
    pub installed_at: i64,
    pub enabled: bool,
}

pub struct ExtensionManager {
    installed_extensions: HashMap<String, InstalledExtension>,
    available_extensions: Vec<ExtensionMetadata>,
}

impl ExtensionManager {
    pub fn new() -> Self {
        Self {
            installed_extensions: HashMap::new(),
            available_extensions: Self::default_available_extensions(),
        }
    }

    fn default_available_extensions() -> Vec<ExtensionMetadata> {
        vec![
            ExtensionMetadata {
                id: "notion".to_string(),
                name: "Notion".to_string(),
                version: "1.0.0".to_string(),
                author: "Notion".to_string(),
                description: "连接到 Notion 以读取和写入页面、数据库和评论".to_string(),
                tools: vec!["read_page".to_string(), "write_page".to_string(), "query_database".to_string()],
                permissions: vec!["notion:read".to_string(), "notion:write".to_string()],
                resource_requirements: None,
                auto_update: Some(true),
            },
            ExtensionMetadata {
                id: "github".to_string(),
                name: "GitHub".to_string(),
                version: "1.0.0".to_string(),
                author: "GitHub".to_string(),
                description: "连接到 GitHub 以进行仓库管理、问题、PR 和代码搜索".to_string(),
                tools: vec!["read_repo".to_string(), "create_issue".to_string(), "create_pr".to_string()],
                permissions: vec!["github:read".to_string(), "github:write".to_string()],
                resource_requirements: None,
                auto_update: Some(true),
            },
            ExtensionMetadata {
                id: "slack".to_string(),
                name: "Slack".to_string(),
                version: "1.0.0".to_string(),
                author: "Slack".to_string(),
                description: "连接到 Slack 以进行消息传递、频道管理和团队沟通".to_string(),
                tools: vec!["send_message".to_string(), "read_channel".to_string(), "list_users".to_string()],
                permissions: vec!["slack:read".to_string(), "slack:write".to_string()],
                resource_requirements: None,
                auto_update: Some(true),
            },
            ExtensionMetadata {
                id: "linear".to_string(),
                name: "Linear".to_string(),
                version: "1.0.0".to_string(),
                author: "Linear".to_string(),
                description: "连接到 Linear 以进行问题跟踪、项目管理和团队工作流".to_string(),
                tools: vec!["create_issue".to_string(), "update_issue".to_string(), "list_issues".to_string()],
                permissions: vec!["linear:read".to_string(), "linear:write".to_string()],
                resource_requirements: None,
                auto_update: Some(true),
            },
            ExtensionMetadata {
                id: "stripe".to_string(),
                name: "Stripe".to_string(),
                version: "1.0.0".to_string(),
                author: "Stripe".to_string(),
                description: "连接到 Stripe 以进行支付处理、订阅和财务数据".to_string(),
                tools: vec!["create_payment".to_string(), "list_invoices".to_string(), "manage_subscription".to_string()],
                permissions: vec!["stripe:read".to_string(), "stripe:write".to_string()],
                resource_requirements: Some(ResourceRequirements {
                    min_memory_mb: Some(256),
                    min_disk_mb: Some(100),
                    required_features: vec!["tls".to_string()],
                }),
                auto_update: Some(true),
            },
        ]
    }

    pub fn install_extension(&mut self, metadata: ExtensionMetadata) -> Result<()> {
        if self.installed_extensions.contains_key(&metadata.id) {
            return Err(Error::StorageError("Extension already installed".to_string()));
        }

        let extension = InstalledExtension {
            metadata: metadata.clone(),
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            enabled: true,
        };

        self.installed_extensions.insert(metadata.id, extension);
        Ok(())
    }

    pub fn uninstall_extension(&mut self, extension_id: &str) -> Result<()> {
        self.installed_extensions
            .remove(extension_id)
            .ok_or(Error::StorageError("Extension not found".to_string()))?;
        Ok(())
    }

    pub fn get_installed_extensions(&self) -> Vec<InstalledExtension> {
        self.installed_extensions.values().cloned().collect()
    }

    pub fn get_extension(&self, extension_id: &str) -> Option<&InstalledExtension> {
        self.installed_extensions.get(extension_id)
    }

    pub fn enable_extension(&mut self, extension_id: &str) -> Result<()> {
        let extension = self
            .installed_extensions
            .get_mut(extension_id)
            .ok_or(Error::StorageError("Extension not found".to_string()))?;
        extension.enabled = true;
        Ok(())
    }

    pub fn disable_extension(&mut self, extension_id: &str) -> Result<()> {
        let extension = self
            .installed_extensions
            .get_mut(extension_id)
            .ok_or(Error::StorageError("Extension not found".to_string()))?;
        extension.enabled = false;
        Ok(())
    }

    pub fn set_available_extensions(&mut self, extensions: Vec<ExtensionMetadata>) {
        self.available_extensions = extensions;
    }

    pub fn get_available_extensions(&self) -> &[ExtensionMetadata] {
        &self.available_extensions
    }

    pub fn search_extensions(&self, query: &str) -> Vec<&ExtensionMetadata> {
        let query_lower = query.to_lowercase();
        self.available_extensions
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&query_lower)
                    || e.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    pub fn get_enabled_tools(&self) -> Vec<String> {
        self.installed_extensions
            .values()
            .filter(|e| e.enabled)
            .flat_map(|e| e.metadata.tools.clone())
            .collect()
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_extension() {
        let mut manager = ExtensionManager::new();
        let metadata = ExtensionMetadata {
            id: "github-ext".to_string(),
            name: "GitHub Extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "GitHub integration".to_string(),
            tools: vec!["read_issues".to_string(), "create_issue".to_string()],
            permissions: vec!["github:read".to_string()],
            resource_requirements: None,
            auto_update: Some(true),
        };

        assert!(manager.install_extension(metadata).is_ok());
        assert_eq!(manager.get_installed_extensions().len(), 1);
    }

    #[test]
    fn test_enable_disable_extension() {
        let mut manager = ExtensionManager::new();
        let metadata = ExtensionMetadata {
            id: "github-ext".to_string(),
            name: "GitHub Extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "GitHub integration".to_string(),
            tools: vec!["read_issues".to_string()],
            permissions: vec![],
            resource_requirements: None,
            auto_update: Some(true),
        };

        manager.install_extension(metadata).unwrap();
        assert!(manager.disable_extension("github-ext").is_ok());
        assert!(!manager.get_extension("github-ext").unwrap().enabled);
        assert!(manager.enable_extension("github-ext").is_ok());
        assert!(manager.get_extension("github-ext").unwrap().enabled);
    }

    #[test]
    fn test_get_enabled_tools() {
        let mut manager = ExtensionManager::new();
        let metadata = ExtensionMetadata {
            id: "github-ext".to_string(),
            name: "GitHub Extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "GitHub integration".to_string(),
            tools: vec!["read_issues".to_string(), "create_issue".to_string()],
            permissions: vec![],
            resource_requirements: None,
            auto_update: Some(true),
        };

        manager.install_extension(metadata).unwrap();
        let tools = manager.get_enabled_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"read_issues".to_string()));
    }
}
