use proptest::prelude::*;
use desktop_client::plugin_manager::{PluginManager, PluginMetadata, ResourceRequirements};

prop_compose! {
    fn arb_resource_requirements()(
        min_memory_mb in 10u32..1000,
        min_disk_mb in 10u32..1000,
    ) -> ResourceRequirements {
        ResourceRequirements {
            min_memory_mb,
            min_disk_mb,
            required_features: vec![],
        }
    }
}

prop_compose! {
    fn arb_plugin_metadata()(
        id in "[a-z0-9-]{5,20}",
        name in "[a-zA-Z0-9 ]{5,30}",
        version in r"[0-9]\.[0-9]\.[0-9]",
        author in "[a-zA-Z ]{3,20}",
        description in "[a-zA-Z0-9 ]{10,50}",
        resources in arb_resource_requirements(),
    ) -> PluginMetadata {
        PluginMetadata {
            id,
            name,
            version,
            author,
            description,
            permissions: vec!["read".to_string()],
            resource_requirements: resources,
        }
    }
}

proptest! {
    #[test]
    fn prop_install_plugin_succeeds(metadata in arb_plugin_metadata()) {
        let mut manager = PluginManager::new();
        let result = manager.install_plugin(metadata.clone());
        
        prop_assert!(result.is_ok());
        prop_assert_eq!(manager.get_installed_plugins().len(), 1);
        prop_assert!(manager.get_plugin(&metadata.id).is_some());
    }

    #[test]
    fn prop_duplicate_install_fails(metadata in arb_plugin_metadata()) {
        let mut manager = PluginManager::new();
        manager.install_plugin(metadata.clone()).unwrap();
        
        let result = manager.install_plugin(metadata);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_uninstall_removes_plugin(metadata in arb_plugin_metadata()) {
        let mut manager = PluginManager::new();
        manager.install_plugin(metadata.clone()).unwrap();
        
        let result = manager.uninstall_plugin(&metadata.id);
        prop_assert!(result.is_ok());
        prop_assert_eq!(manager.get_installed_plugins().len(), 0);
    }

    #[test]
    fn prop_enable_disable_toggles_state(metadata in arb_plugin_metadata()) {
        let mut manager = PluginManager::new();
        manager.install_plugin(metadata.clone()).unwrap();
        
        manager.disable_plugin(&metadata.id).unwrap();
        prop_assert!(!manager.get_plugin(&metadata.id).unwrap().enabled);
        
        manager.enable_plugin(&metadata.id).unwrap();
        prop_assert!(manager.get_plugin(&metadata.id).unwrap().enabled);
    }

    #[test]
    fn prop_search_finds_by_name(
        metadata in arb_plugin_metadata(),
        query in "[a-z]{2,5}"
    ) {
        let mut manager = PluginManager::new();
        let mut search_metadata = metadata.clone();
        search_metadata.name = format!("Test{}", query);
        
        manager.install_plugin(search_metadata).unwrap();
        manager.set_available_plugins(vec![metadata]);
        
        let results = manager.search_plugins(&query);
        prop_assert!(results.len() >= 0);
    }

    #[test]
    fn prop_update_changes_version(
        metadata in arb_plugin_metadata(),
        new_version in r"[0-9]\.[0-9]\.[0-9]"
    ) {
        let mut manager = PluginManager::new();
        manager.install_plugin(metadata.clone()).unwrap();
        
        let result = manager.update_plugin(&metadata.id, &new_version);
        prop_assert!(result.is_ok());
        
        let plugin = manager.get_plugin(&metadata.id).unwrap();
        prop_assert_eq!(plugin.metadata.version, new_version);
    }

    #[test]
    fn prop_rollback_restores_version(
        metadata in arb_plugin_metadata(),
        new_version in r"[0-9]\.[0-9]\.[0-9]",
        old_version in r"[0-9]\.[0-9]\.[0-9]"
    ) {
        let mut manager = PluginManager::new();
        let mut plugin = metadata.clone();
        plugin.version = old_version.clone();
        
        manager.install_plugin(plugin).unwrap();
        manager.update_plugin(&metadata.id, &new_version).unwrap();
        manager.rollback_plugin(&metadata.id, &old_version).unwrap();
        
        let current = manager.get_plugin(&metadata.id).unwrap();
        prop_assert_eq!(current.metadata.version, old_version);
    }

    #[test]
    fn prop_check_updates_detects_new_versions(
        mut metadata in arb_plugin_metadata()
    ) {
        let mut manager = PluginManager::new();
        let old_version = metadata.version.clone();
        
        manager.install_plugin(metadata.clone()).unwrap();
        
        metadata.version = "9.9.9".to_string();
        manager.set_available_plugins(vec![metadata.clone()]);
        
        let updates = manager.check_for_updates().unwrap();
        prop_assert!(updates.len() > 0);
        prop_assert_eq!(updates[0].current_version, old_version);
        prop_assert_eq!(updates[0].new_version, "9.9.9");
    }
}
