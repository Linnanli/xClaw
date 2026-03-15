use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tools: Vec<String>,
    pub permissions: Vec<String>,
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
            available_extensions: Vec::new(),
        }
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
        };

        manager.install_extension(metadata).unwrap();
        let tools = manager.get_enabled_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"read_issues".to_string()));
    }
}
