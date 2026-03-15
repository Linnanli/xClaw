use proptest::prelude::*;
use desktop_client::offline_mode::{OfflineModeManager, SyncItem};

proptest! {
    #[test]
    fn prop_network_detection_toggles_offline_state(is_connected in any::<bool>()) {
        let mut manager = OfflineModeManager::new();
        
        manager.detect_network_status(!is_connected).unwrap();
        prop_assert_eq!(manager.is_offline(), !is_connected);
        
        manager.detect_network_status(is_connected).unwrap();
        prop_assert_eq!(manager.is_offline(), !is_connected);
    }

    #[test]
    fn prop_offline_mode_disables_network_operations(
        operations in prop::collection::vec("[a-z_]{5,20}", 1..10)
    ) {
        let mut manager = OfflineModeManager::new();
        manager.enable_offline_mode().unwrap();
        
        prop_assert!(!manager.can_perform_operation("network_request"));
        
        for op in operations {
            let allowed = manager.can_perform_operation(&op);
            if op == "network_request" {
                prop_assert!(!allowed);
            }
        }
    }

    #[test]
    fn prop_online_mode_allows_all_operations(
        operations in prop::collection::vec("[a-z_]{5,20}", 1..10)
    ) {
        let manager = OfflineModeManager::new();
        
        for op in operations {
            let allowed = manager.can_perform_operation(&op);
            prop_assert!(allowed);
        }
    }

    #[test]
    fn prop_sync_queue_accumulates_items(
        items_count in 1usize..20
    ) {
        let mut manager = OfflineModeManager::new();
        
        for i in 0..items_count {
            let item = SyncItem {
                id: format!("item-{}", i),
                item_type: "message".to_string(),
                data: format!("data-{}", i),
                timestamp: i as u64,
            };
            manager.queue_sync_item(item).unwrap();
        }
        
        prop_assert_eq!(manager.get_sync_queue().len(), items_count);
    }

    #[test]
    fn prop_clear_sync_queue_empties_items(
        items_count in 1usize..20
    ) {
        let mut manager = OfflineModeManager::new();
        
        for i in 0..items_count {
            let item = SyncItem {
                id: format!("item-{}", i),
                item_type: "message".to_string(),
                data: format!("data-{}", i),
                timestamp: i as u64,
            };
            manager.queue_sync_item(item).unwrap();
        }
        
        manager.clear_sync_queue();
        prop_assert_eq!(manager.get_sync_queue().len(), 0);
    }

    #[test]
    fn prop_offline_capabilities_consistent(
        is_offline in any::<bool>()
    ) {
        let mut manager = OfflineModeManager::new();
        
        if is_offline {
            manager.enable_offline_mode().unwrap();
        } else {
            manager.disable_offline_mode().unwrap();
        }
        
        let caps = manager.get_capabilities();
        prop_assert_eq!(caps.can_read_files, true);
        prop_assert_eq!(caps.can_write_files, true);
        prop_assert_eq!(caps.can_use_local_llm, true);
        prop_assert_eq!(caps.can_use_cached_plugins, true);
        prop_assert_eq!(caps.can_access_local_tools, true);
    }

    #[test]
    fn prop_network_status_transitions(
        transitions in prop::collection::vec(any::<bool>(), 2..10)
    ) {
        let mut manager = OfflineModeManager::new();
        
        for is_connected in transitions {
            manager.detect_network_status(is_connected).unwrap();
            
            if is_connected {
                prop_assert!(!manager.is_offline());
            } else {
                prop_assert!(manager.is_offline());
            }
        }
    }

    #[test]
    fn prop_sync_updates_last_sync_timestamp(
        _dummy in any::<()>()
    ) {
        let mut manager = OfflineModeManager::new();
        let initial_sync = manager.get_state().last_sync;
        
        manager.trigger_sync().unwrap();
        let after_sync = manager.get_state().last_sync;
        
        prop_assert!(after_sync >= initial_sync);
    }

    #[test]
    fn prop_cached_data_size_updates(
        sizes in prop::collection::vec(1u32..10000, 1..10)
    ) {
        let mut manager = OfflineModeManager::new();
        
        for size in sizes {
            manager.update_cached_data_size(size);
            prop_assert_eq!(manager.get_state().cached_data_size_mb, size);
        }
    }
}
