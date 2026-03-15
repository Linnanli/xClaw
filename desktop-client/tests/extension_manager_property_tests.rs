use desktop_client::extension_manager::{ExtensionManager, ExtensionMetadata};
use proptest::prelude::*;

prop_compose! {
    fn arb_extension_metadata()(
        id in "ext-[a-z0-9]{5}",
        name in "[A-Z][a-z]{3,10}",
        version in r"[0-9]\.[0-9]\.[0-9]",
        author in "[A-Z][a-z]{3,10}",
        description in "[A-Za-z ]{10,50}",
        tools_count in 0..5usize,
        perms_count in 0..3usize,
    ) -> ExtensionMetadata {
        ExtensionMetadata {
            id,
            name,
            version,
            author,
            description,
            tools: (0..tools_count).map(|i| format!("tool_{}", i)).collect(),
            permissions: (0..perms_count).map(|i| format!("perm_{}", i)).collect(),
        }
    }
}

proptest! {
    #[test]
    fn prop_install_extension_succeeds(metadata in arb_extension_metadata()) {
        let mut manager = ExtensionManager::new();
        let result = manager.install_extension(metadata.clone());
        prop_assert!(result.is_ok());
        prop_assert_eq!(manager.get_installed_extensions().len(), 1);
    }

    #[test]
    fn prop_duplicate_install_fails(metadata in arb_extension_metadata()) {
        let mut manager = ExtensionManager::new();
        manager.install_extension(metadata.clone()).unwrap();
        let result = manager.install_extension(metadata);
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_uninstall_removes_extension(metadata in arb_extension_metadata()) {
        let mut manager = ExtensionManager::new();
        let id = metadata.id.clone();
        manager.install_extension(metadata).unwrap();
        manager.uninstall_extension(&id).unwrap();
        prop_assert_eq!(manager.get_installed_extensions().len(), 0);
    }

    #[test]
    fn prop_enable_disable_toggles_status(metadata in arb_extension_metadata()) {
        let mut manager = ExtensionManager::new();
        let id = metadata.id.clone();
        manager.install_extension(metadata).unwrap();
        
        manager.disable_extension(&id).unwrap();
        prop_assert!(!manager.get_extension(&id).unwrap().enabled);
        
        manager.enable_extension(&id).unwrap();
        prop_assert!(manager.get_extension(&id).unwrap().enabled);
    }

    #[test]
    fn prop_search_finds_by_name(metadata in arb_extension_metadata()) {
        let mut manager = ExtensionManager::new();
        let name = metadata.name.clone();
        manager.install_extension(metadata).unwrap();
        
        let results = manager.search_extensions(&name);
        prop_assert_eq!(results.len(), 1);
    }

    #[test]
    fn prop_enabled_tools_only_from_enabled_extensions(
        metadata1 in arb_extension_metadata(),
        metadata2 in arb_extension_metadata(),
    ) {
        let mut manager = ExtensionManager::new();
        let id1 = metadata1.id.clone();
        let id2 = metadata2.id.clone();
        
        manager.install_extension(metadata1).unwrap();
        manager.install_extension(metadata2).unwrap();
        manager.disable_extension(&id1).unwrap();
        
        let tools = manager.get_enabled_tools();
        prop_assert!(tools.len() > 0);
    }

    #[test]
    fn prop_multiple_extensions_managed(
        metadata1 in arb_extension_metadata(),
        metadata2 in arb_extension_metadata(),
        metadata3 in arb_extension_metadata(),
    ) {
        let mut manager = ExtensionManager::new();
        manager.install_extension(metadata1).unwrap();
        manager.install_extension(metadata2).unwrap();
        manager.install_extension(metadata3).unwrap();
        
        prop_assert_eq!(manager.get_installed_extensions().len(), 3);
    }

    #[test]
    fn prop_available_extensions_independent(
        metadata1 in arb_extension_metadata(),
        metadata2 in arb_extension_metadata(),
    ) {
        let mut manager = ExtensionManager::new();
        manager.set_available_extensions(vec![metadata1, metadata2]);
        
        prop_assert_eq!(manager.get_available_extensions().len(), 2);
        prop_assert_eq!(manager.get_installed_extensions().len(), 0);
    }
}
