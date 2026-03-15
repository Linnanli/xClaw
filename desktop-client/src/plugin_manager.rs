use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub resource_requirements: ResourceRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub min_memory_mb: u32,
    pub min_disk_mb: u32,
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub metadata: PluginMetadata,
    pub installed_at: i64,
    pub enabled: bool,
    pub auto_update: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUpdate {
    pub plugin_id: String,
    pub current_version: String,
    pub new_version: String,
    pub changelog: String,
    pub size_mb: u32,
}

pub struct PluginManager {
    installed_plugins: HashMap<String, InstalledPlugin>,
    available_plugins: Vec<PluginMetadata>,
    pending_updates: Vec<PluginUpdate>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            installed_plugins: HashMap::new(),
            available_plugins: Vec::new(),
            pending_updates: Vec::new(),
        }
    }

    pub fn install_plugin(&mut self, metadata: PluginMetadata) -> Result<()> {
        if self.installed_plugins.contains_key(&metadata.id) {
            return Err(Error::StorageError("Plugin already installed".to_string()));
        }

        let plugin = InstalledPlugin {
            metadata: metadata.clone(),
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            enabled: true,
            auto_update: true,
        };

        self.installed_plugins.insert(metadata.id, plugin);
        Ok(())
    }

    pub fn uninstall_plugin(&mut self, plugin_id: &str) -> Result<()> {
        self.installed_plugins
            .remove(plugin_id)
            .ok_or(Error::StorageError("Plugin not found".to_string()))?;
        Ok(())
    }

    pub fn get_installed_plugins(&self) -> Vec<InstalledPlugin> {
        self.installed_plugins.values().cloned().collect()
    }

    pub fn get_plugin(&self, plugin_id: &str) -> Option<&InstalledPlugin> {
        self.installed_plugins.get(plugin_id)
    }

    pub fn enable_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let plugin = self
            .installed_plugins
            .get_mut(plugin_id)
            .ok_or(Error::StorageError("Plugin not found".to_string()))?;
        plugin.enabled = true;
        Ok(())
    }

    pub fn disable_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let plugin = self
            .installed_plugins
            .get_mut(plugin_id)
            .ok_or(Error::StorageError("Plugin not found".to_string()))?;
        plugin.enabled = false;
        Ok(())
    }

    pub fn set_available_plugins(&mut self, plugins: Vec<PluginMetadata>) {
        self.available_plugins = plugins;
    }

    pub fn get_available_plugins(&self) -> &[PluginMetadata] {
        &self.available_plugins
    }

    pub fn check_for_updates(&mut self) -> Result<Vec<PluginUpdate>> {
        let mut updates = Vec::new();

        for (_, installed) in &self.installed_plugins {
            for available in &self.available_plugins {
                if installed.metadata.id == available.id
                    && installed.metadata.version != available.version
                {
                    updates.push(PluginUpdate {
                        plugin_id: available.id.clone(),
                        current_version: installed.metadata.version.clone(),
                        new_version: available.version.clone(),
                        changelog: format!("Updated to {}", available.version),
                        size_mb: 10, // Placeholder
                    });
                }
            }
        }

        self.pending_updates = updates.clone();
        Ok(updates)
    }

    pub fn get_pending_updates(&self) -> &[PluginUpdate] {
        &self.pending_updates
    }

    pub fn update_plugin(&mut self, plugin_id: &str, new_version: &str) -> Result<()> {
        let plugin = self
            .installed_plugins
            .get_mut(plugin_id)
            .ok_or(Error::StorageError("Plugin not found".to_string()))?;

        plugin.metadata.version = new_version.to_string();
        self.pending_updates.retain(|u| u.plugin_id != plugin_id);
        Ok(())
    }

    pub fn rollback_plugin(&mut self, plugin_id: &str, previous_version: &str) -> Result<()> {
        let plugin = self
            .installed_plugins
            .get_mut(plugin_id)
            .ok_or(Error::StorageError("Plugin not found".to_string()))?;

        plugin.metadata.version = previous_version.to_string();
        Ok(())
    }

    pub fn search_plugins(&self, query: &str) -> Vec<&PluginMetadata> {
        let query_lower = query.to_lowercase();
        self.available_plugins
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_plugin() {
        let mut manager = PluginManager::new();
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "A test plugin".to_string(),
            permissions: vec!["read".to_string()],
            resource_requirements: ResourceRequirements {
                min_memory_mb: 100,
                min_disk_mb: 50,
                required_features: vec![],
            },
        };

        assert!(manager.install_plugin(metadata).is_ok());
        assert_eq!(manager.get_installed_plugins().len(), 1);
    }

    #[test]
    fn test_uninstall_plugin() {
        let mut manager = PluginManager::new();
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "A test plugin".to_string(),
            permissions: vec![],
            resource_requirements: ResourceRequirements {
                min_memory_mb: 100,
                min_disk_mb: 50,
                required_features: vec![],
            },
        };

        manager.install_plugin(metadata).unwrap();
        assert!(manager.uninstall_plugin("test-plugin").is_ok());
        assert_eq!(manager.get_installed_plugins().len(), 0);
    }

    #[test]
    fn test_enable_disable_plugin() {
        let mut manager = PluginManager::new();
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            description: "A test plugin".to_string(),
            permissions: vec![],
            resource_requirements: ResourceRequirements {
                min_memory_mb: 100,
                min_disk_mb: 50,
                required_features: vec![],
            },
        };

        manager.install_plugin(metadata).unwrap();
        assert!(manager.disable_plugin("test-plugin").is_ok());
        assert!(!manager.get_plugin("test-plugin").unwrap().enabled);
        assert!(manager.enable_plugin("test-plugin").is_ok());
        assert!(manager.get_plugin("test-plugin").unwrap().enabled);
    }
}
