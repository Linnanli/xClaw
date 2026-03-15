use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineModeState {
    pub is_offline: bool,
    pub last_sync: u64,
    pub cached_data_size_mb: u32,
    pub network_status: NetworkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkStatus {
    Online,
    Offline,
    Reconnecting,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineCapabilities {
    pub can_read_files: bool,
    pub can_write_files: bool,
    pub can_use_local_llm: bool,
    pub can_use_cached_plugins: bool,
    pub can_access_local_tools: bool,
}

pub struct OfflineModeManager {
    state: OfflineModeState,
    capabilities: OfflineCapabilities,
    sync_queue: Vec<SyncItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncItem {
    pub id: String,
    pub item_type: String,
    pub data: String,
    pub timestamp: u64,
}

impl OfflineModeManager {
    pub fn new() -> Self {
        Self {
            state: OfflineModeState {
                is_offline: false,
                last_sync: 0,
                cached_data_size_mb: 0,
                network_status: NetworkStatus::Online,
            },
            capabilities: OfflineCapabilities {
                can_read_files: true,
                can_write_files: true,
                can_use_local_llm: true,
                can_use_cached_plugins: true,
                can_access_local_tools: true,
            },
            sync_queue: Vec::new(),
        }
    }

    pub fn detect_network_status(&mut self, is_connected: bool) -> Result<()> {
        if is_connected && self.state.is_offline {
            // Transitioning from offline to online
            self.state.is_offline = false;
            self.state.network_status = NetworkStatus::Reconnecting;
            self.trigger_sync()?;
        } else if !is_connected && !self.state.is_offline {
            // Transitioning from online to offline
            self.state.is_offline = true;
            self.state.network_status = NetworkStatus::Offline;
        }

        Ok(())
    }

    pub fn get_state(&self) -> &OfflineModeState {
        &self.state
    }

    pub fn get_capabilities(&self) -> &OfflineCapabilities {
        &self.capabilities
    }

    pub fn is_offline(&self) -> bool {
        self.state.is_offline
    }

    pub fn can_perform_operation(&self, operation: &str) -> bool {
        if !self.state.is_offline {
            return true;
        }

        match operation {
            "read_file" => self.capabilities.can_read_files,
            "write_file" => self.capabilities.can_write_files,
            "use_llm" => self.capabilities.can_use_local_llm,
            "use_plugin" => self.capabilities.can_use_cached_plugins,
            "use_tool" => self.capabilities.can_access_local_tools,
            "network_request" => false, // Network operations not allowed offline
            _ => false,
        }
    }

    pub fn queue_sync_item(&mut self, item: SyncItem) -> Result<()> {
        self.sync_queue.push(item);
        Ok(())
    }

    pub fn get_sync_queue(&self) -> &[SyncItem] {
        &self.sync_queue
    }

    pub fn clear_sync_queue(&mut self) {
        self.sync_queue.clear();
    }

    pub fn trigger_sync(&mut self) -> Result<()> {
        self.state.last_sync = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.state.network_status = NetworkStatus::Online;
        Ok(())
    }

    pub fn update_cached_data_size(&mut self, size_mb: u32) {
        self.state.cached_data_size_mb = size_mb;
    }

    pub fn enable_offline_mode(&mut self) -> Result<()> {
        self.state.is_offline = true;
        self.state.network_status = NetworkStatus::Offline;
        Ok(())
    }

    pub fn disable_offline_mode(&mut self) -> Result<()> {
        self.state.is_offline = false;
        self.state.network_status = NetworkStatus::Online;
        Ok(())
    }

    pub fn get_network_status(&self) -> &NetworkStatus {
        &self.state.network_status
    }
}

impl Default for OfflineModeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_mode_detection() {
        let mut manager = OfflineModeManager::new();
        assert!(!manager.is_offline());

        manager.detect_network_status(false).unwrap();
        assert!(manager.is_offline());

        manager.detect_network_status(true).unwrap();
        assert!(!manager.is_offline());
    }

    #[test]
    fn test_offline_capabilities() {
        let manager = OfflineModeManager::new();
        let caps = manager.get_capabilities();

        assert!(caps.can_read_files);
        assert!(caps.can_write_files);
        assert!(caps.can_use_local_llm);
        assert!(caps.can_use_cached_plugins);
        assert!(caps.can_access_local_tools);
    }

    #[test]
    fn test_operation_allowed_offline() {
        let mut manager = OfflineModeManager::new();
        manager.enable_offline_mode().unwrap();

        assert!(manager.can_perform_operation("read_file"));
        assert!(manager.can_perform_operation("write_file"));
        assert!(manager.can_perform_operation("use_llm"));
        assert!(!manager.can_perform_operation("network_request"));
    }

    #[test]
    fn test_sync_queue() {
        let mut manager = OfflineModeManager::new();
        let item = SyncItem {
            id: "1".to_string(),
            item_type: "message".to_string(),
            data: "test".to_string(),
            timestamp: 0,
        };

        manager.queue_sync_item(item).unwrap();
        assert_eq!(manager.get_sync_queue().len(), 1);

        manager.clear_sync_queue();
        assert_eq!(manager.get_sync_queue().len(), 0);
    }
}
